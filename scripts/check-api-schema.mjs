import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import Ajv2020 from "ajv/dist/2020.js";
import addFormats from "ajv-formats";
import openapiTS from "openapi-typescript";

const document = JSON.parse(
  await readFile(new URL("../contracts/openapi.json", import.meta.url), "utf8"),
);
assert.equal(document.openapi, "3.1.0");
// Component refs are document-relative in OpenAPI, so relocate them with their schema definitions.
const definitions = JSON.parse(
  JSON.stringify(document.components.schemas, (key, value) =>
    key === "$ref" && typeof value === "string"
      ? value.replace(/^#\/components\/schemas\//, "#/$defs/")
      : value,
  ),
);
const ajv = new Ajv2020({ strict: true, allErrors: true });
addFormats(ajv);
// These OpenAPI formats are annotations, not additional JavaScript number range guarantees.
for (const format of ["int32", "int64", "binary"]) ajv.addFormat(format, true);
const validators = new Map(
  Object.keys(definitions).map((name) => [
    name,
    ajv.compile({ $defs: definitions, $ref: `#/$defs/${name}` }),
  ]),
);
const uuid = "00000000-0000-0000-0000-000000000001";
const record = { id: uuid, updatedAt: 0, isDeleted: false, body: null };
const published = { ownerId: uuid, serverVersion: "9007199254740993", record };
const message = {
  id: uuid,
  owner_id: uuid,
  conversation_id: uuid,
  parent_message_id: null,
  role: "user",
  content: "",
  toc_line: "",
  created_order: 1,
};
const { parent_message_id: _parent, ...messageWithoutParent } = message;
const { toc_line: _tocLine, ...messageWithoutTocLine } = message;
for (const [name, valid, invalid] of [
  [
    "Cursor",
    ["0", "9007199254740993", "9223372036854775807"],
    [0, "01", "-1", null],
  ],
  [
    "Record",
    [record, { ...record, body: [1, "text", false] }],
    [
      { id: uuid, updatedAt: 0, isDeleted: false },
      { ...record, ownerId: uuid },
    ],
  ],
  [
    "Message",
    [message, { ...message, content: "内容", toc_line: "内容" }],
    [
      messageWithoutParent,
      messageWithoutTocLine,
      { ...message, toc_line: null },
      { ...message, toc_line: 42 },
    ],
  ],
  [
    "UploadResult",
    [
      { status: "accepted", record: published },
      { status: "retained", record: published },
    ],
    [
      { status: "unknown", record: published },
      { status: "accepted", record: { ...published, serverVersion: 42 } },
    ],
  ],
  [
    "InputErrorResponse",
    [{ kind: "field", path: "title", message: "invalid" }],
    [{ error: "not_found", login: "" }],
  ],
  [
    "ErrorResponse",
    [{ error: "authentication_required", login: "/auth/login" }],
    [{ error: "authentication_required" }],
  ],
]) {
  const validate = validators.get(name);
  for (const value of valid)
    assert.equal(
      validate(value),
      true,
      `${name}: ${JSON.stringify(validate.errors)}`,
    );
  for (const value of invalid)
    assert.equal(validate(value), false, `${name}: ${JSON.stringify(value)}`);
}
// Traverse the entire document: references in responses and parameters must resolve too.
const visit = (value) => {
  if (value === null || typeof value !== "object") return;
  if ("$ref" in value) {
    assert.ok(
      value.$ref.startsWith("#/"),
      `Unexpected external ref: ${value.$ref}`,
    );
    let resolved = document;
    for (const part of value.$ref.slice(2).split("/"))
      resolved = resolved?.[part.replace(/~1/g, "/").replace(/~0/g, "~")];
    assert.notEqual(resolved, undefined, value.$ref);
  }
  for (const child of Object.values(value)) visit(child);
};
visit(document);
// Parsing the production document through the selected generator catches unsupported constructs.
await openapiTS(document);
console.log(
  `Validated ${validators.size} production schemas and every local OpenAPI reference.`,
);
