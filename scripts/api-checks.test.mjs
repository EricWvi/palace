import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtemp, readFile, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { staleArtifacts } from "./api-artifacts.mjs";
import { responseValidator } from "./api-responses.mjs";
import { browserTypes } from "./api-types.mjs";

const document = JSON.parse(
  await readFile(new URL("../contracts/openapi.json", import.meta.url), "utf8"),
);

test("generation checks detect missing and stale files without overwriting local changes", async () => {
  const root = await mkdtemp(path.join(tmpdir(), "palace-api-check-"));
  try {
    const artifacts = new Map([
      ["contract.json", "expected"],
      ["api.ts", "generated"],
    ]);
    await writeFile(path.join(root, "contract.json"), "local edit");
    assert.deepEqual(await staleArtifacts(root, artifacts), [
      "contract.json",
      "api.ts",
    ]);
    assert.equal(
      await readFile(path.join(root, "contract.json"), "utf8"),
      "local edit",
    );
    await assert.rejects(readFile(path.join(root, "api.ts")), {
      code: "ENOENT",
    });
    await writeFile(path.join(root, "contract.json"), "expected");
    await writeFile(path.join(root, "api.ts"), "generated");
    assert.deepEqual(await staleArtifacts(root, artifacts), []);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("browser generation is deterministic and never removes Origin from the source contract", async () => {
  const before = structuredClone(document);
  const output = await browserTypes(document);
  assert.equal(await browserTypes(document), output);
  assert.deepEqual(document, before);
  assert.match(output, /history: Blob/);
});

test("response validation fails on undocumented operations, statuses, media and payload drift", () => {
  const validate = responseValidator(document);
  const sample = {
    method: "GET",
    path: "/api/me",
    status: 401,
    headers: { "content-type": "application/json; charset=utf-8" },
    body: JSON.stringify({
      error: "authentication_required",
      login: "/auth/login",
    }),
  };
  validate(sample);
  for (const change of [
    { path: "/api/forgotten" },
    { method: "PATCH" },
    { status: 418 },
    { headers: { "content-type": "text/html" } },
    { body: '{"error":"authentication_required"}' },
    { body: '{"error":"unknown","login":""}' },
    { body: "not json" },
  ])
    assert.throws(() => validate({ ...sample, ...change }));
  validate({
    ...sample,
    path: "/api/sync",
    status: 400,
    headers: { "content-type": "text/plain; charset=utf-8" },
    body: "Invalid query",
  });
  validate({
    ...sample,
    path: "/auth/login",
    status: 303,
    headers: {},
    body: "",
  });
  assert.throws(() =>
    validate({ ...sample, path: "/auth/login", status: 303 }),
  );
});

test("real-response selection validates nested nullable fields and exact string cursors", () => {
  const validate = responseValidator(document);
  const uuid = "00000000-0000-0000-0000-000000000001";
  const message = {
    id: uuid,
    owner_id: uuid,
    conversation_id: uuid,
    parent_message_id: null,
    role: "user",
    content: "",
    created_order: 1,
  };
  const sample = {
    method: "GET",
    path: "/api/conversations/{id}/paths/{path_id}",
    status: 200,
    headers: { "content-type": "application/json" },
    body: JSON.stringify([message]),
  };
  validate(sample);
  const { parent_message_id: _parent, ...missing } = message;
  assert.equal(_parent, null);
  assert.throws(() => validate({ ...sample, body: JSON.stringify([missing]) }));
  validate({
    ...sample,
    path: "/api/sync",
    body: '{"records":[],"cursor":"9007199254740993"}',
  });
  assert.throws(() =>
    validate({
      ...sample,
      path: "/api/sync",
      body: '{"records":[],"cursor":9007199254740993}',
    }),
  );
});
