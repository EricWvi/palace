import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { copyFile, mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";
import Ajv2020 from "ajv/dist/2020.js";
import addFormats from "ajv-formats";
import openapiTS, { astToString } from "openapi-typescript";
import ts from "typescript";

const root = fileURLToPath(new URL("../", import.meta.url));
const temporaryRoot = path.join(root, ".tmp");
await mkdir(temporaryRoot, { recursive: true });
const temporary = await mkdtemp(path.join(temporaryRoot, "contract-probe-"));

try {
  const fixture = JSON.parse(
    execFileSync(
      "cargo",
      [
        "run",
        "--locked",
        "--quiet",
        "-p",
        "palace-backend",
        "--example",
        "contract_probe",
      ],
      { cwd: root, encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] },
    ),
  );
  assert.equal(fixture.openapi.openapi, "3.1.0");
  assert.equal(fixture.missing_body_rejected, true);
  assert.deepEqual(fixture.invalid_cursors_rejected, [
    true,
    true,
    true,
    true,
    true,
    true,
  ]);
  assert.equal(fixture.multibyte_title_rejected, true);
  assert.deepEqual(fixture.null_body, {
    id: "00000000-0000-0000-0000-000000000001",
    updatedAt: 0,
    isDeleted: false,
    body: null,
  });
  assert.equal(fixture.samples.accepted.record.cursor, "9007199254740993");
  assert.equal(fixture.samples.retained.record.cursor, "9223372036854775807");

  // Validate extracted schemas, not the OpenAPI document as if it were a JSON Schema.
  const definitions = JSON.parse(
    JSON.stringify(fixture.openapi.components.schemas, (key, value) =>
      key === "$ref" && typeof value === "string"
        ? value.replace(/^#\/components\/schemas\//, "#/$defs/")
        : value,
    ),
  );
  const ajv = new Ajv2020({ strict: true, allErrors: true });
  addFormats(ajv);
  // OpenAPI's binary format describes a transport part; AJV only checks its string representation.
  ajv.addFormat("binary", true);
  const validateOutcome = ajv.compile({
    $defs: definitions,
    $ref: "#/$defs/Outcome",
  });
  for (const sample of Object.values(fixture.samples)) {
    assert.equal(
      validateOutcome(sample),
      true,
      JSON.stringify(validateOutcome.errors),
    );
  }
  for (const invalid of [
    { status: "unknown", record: fixture.samples.accepted.record },
    { status: "accepted" },
    { status: "accepted", record: { cursor: "0" } },
    { status: "accepted", record: { cursor: 42, parent_message_id: null } },
    { status: "accepted", record: { cursor: "01", parent_message_id: null } },
    { status: "accepted", record: { cursor: "-1", parent_message_id: null } },
    {
      status: "accepted",
      record: { cursor: "0", parent_message_id: "not-a-uuid" },
    },
    {
      status: "accepted",
      record: { ...fixture.samples.accepted.record, extra: true },
    },
  ]) {
    assert.equal(validateOutcome(invalid), false, JSON.stringify(invalid));
  }
  const validateMultipart = ajv.compile({
    $defs: definitions,
    $ref: "#/$defs/FileImport",
  });
  assert.equal(validateMultipart(fixture.multipart), true);
  assert.equal(validateMultipart({ title: "missing fields" }), false);
  assert.equal(validateMultipart({ ...fixture.multipart, extra: true }), false);

  const schemaFile = path.join(temporary, "schema.ts");
  await writeFile(
    schemaFile,
    astToString(
      await openapiTS(fixture.openapi, {
        // Binary parts must accept browser File/Blob values instead of generated string placeholders.
        transform(schema) {
          if (schema.format === "binary") {
            return ts.factory.createTypeReferenceNode("Blob");
          }
        },
      }),
    ),
  );
  const checkFile = path.join(temporary, "contract-probe.ts");
  await copyFile(
    path.join(root, "scripts", "fixtures", "contract-probe.ts"),
    checkFile,
  );
  const program = ts.createProgram([schemaFile, checkFile], {
    target: ts.ScriptTarget.ES2022,
    module: ts.ModuleKind.NodeNext,
    moduleResolution: ts.ModuleResolutionKind.NodeNext,
    strict: true,
    noEmit: true,
    skipLibCheck: true,
    allowImportingTsExtensions: true,
    types: ["node"],
  });
  const diagnostics = ts.getPreEmitDiagnostics(program);
  assert.equal(
    diagnostics.length,
    0,
    ts.formatDiagnosticsWithColorAndContext(diagnostics, {
      getCanonicalFileName: (name) => name,
      getCurrentDirectory: () => root,
      getNewLine: () => "\n",
    }),
  );
  // The generator requires TS 5, but its output must also compile under the application's TS 6.
  const webRequire = createRequire(
    path.join(root, "apps", "palace-web", "package.json"),
  );
  execFileSync(
    process.execPath,
    [
      webRequire.resolve("typescript/bin/tsc"),
      "--ignoreConfig",
      "--noEmit",
      "--strict",
      "--skipLibCheck",
      "--target",
      "ES2022",
      "--module",
      "NodeNext",
      "--moduleResolution",
      "NodeNext",
      "--allowImportingTsExtensions",
      "--types",
      "node",
      schemaFile,
      checkFile,
    ],
    { cwd: root, stdio: "inherit" },
  );
  execFileSync(process.execPath, [checkFile], { cwd: root, stdio: "inherit" });
  console.log(
    "Contract toolchain verified: OpenAPI 3.1, AJV 2020-12, TypeScript and multipart client.",
  );
} finally {
  await rm(temporary, { recursive: true, force: true });
}
