// Copied beside generated schema.ts by check-contract-toolchain.mjs; never imported by the app.
import assert from "node:assert/strict";
import createClient from "openapi-fetch";
import type { components, paths } from "./schema.ts";

type Envelope = components["schemas"]["CursorEnvelope"];
type Outcome = components["schemas"]["Outcome"];

// These assignments fail if generation loses string, required-nullable or union constraints.
const valid: Envelope = { cursor: "9007199254740993", parent_message_id: null };
// @ts-expect-error A wire cursor cannot be represented as a number.
const numeric: Envelope = { cursor: 42, parent_message_id: null };
// @ts-expect-error Null is accepted, omission is not.
const missing: Envelope = { cursor: "0" };
// @ts-expect-error Only the declared outcome variants are accepted.
const unknown: Outcome = { status: "unknown", record: valid };
void [numeric, missing, unknown];

let requests = 0;
const client = createClient<paths>({
  baseUrl: "https://probe.invalid",
  credentials: "same-origin",
  fetch: async (request) => {
    requests += 1;
    assert.equal(request.method, "POST");
    assert.equal(new URL(request.url).pathname, "/probe/import");
    assert.equal(request.credentials, "same-origin");
    assert.match(
      request.headers.get("content-type") ?? "",
      /^multipart\/form-data; boundary=/,
    );
    const form = await request.formData();
    const history = form.get("history");
    assert.ok(history instanceof File);
    assert.equal(await history.text(), '[{"role":"user","content":"hello"}]');
    assert.deepEqual(
      Object.fromEntries(
        [...form.entries()].filter(([name]) => name !== "history"),
      ),
      {
        title: "示例",
        occurred_at: "0",
        source: "chatgpt",
        session_id: "session-1",
        idempotency_key: "probe-1",
      },
    );
    return Response.json({ status: "accepted", record: valid });
  },
});

const result = await client.POST("/probe/import", {
  body: {
    title: "示例",
    occurred_at: "0",
    source: "chatgpt",
    session_id: "session-1",
    history: new File(['[{"role":"user","content":"hello"}]'], "history.json", {
      type: "application/json",
    }),
    idempotency_key: "probe-1",
  },
  bodySerializer(body) {
    const form = new FormData();
    for (const [name, value] of Object.entries(body)) {
      form.set(name, value);
    }
    return form;
  },
});
assert.equal(requests, 1);
assert.deepEqual(result.data, { status: "accepted", record: valid });
if (result.data?.status === "accepted") {
  const cursor: string = result.data.record.cursor;
  assert.equal(cursor, "9007199254740993");
}
