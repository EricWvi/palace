import { expect, expectTypeOf, it, vi } from "vitest";
import { api, apiData, ApiError, libraryOptions, serializeImport } from "./api";
import type { components } from "./generated/api";
import { testRequest } from "./test-request";

it("sends same-origin credentials and serializes typed path parameters", async () => {
  const fetch = vi
    .spyOn(globalThis, "fetch")
    .mockResolvedValue(Response.json([]));
  expect(await libraryOptions.queryFn()).toEqual([]);
  const request = testRequest(fetch.mock.calls[0][0]);
  expect([request.method, request.url, request.credentials]).toEqual([
    "GET",
    new URL("/api/conversations", window.location.origin).href,
    "same-origin",
  ]);
  fetch.mockResolvedValueOnce(Response.json({ id: "a/b" }));
  expect(
    await apiData(
      api.DELETE("/api/conversations/{id}", {
        params: { path: { id: "a/b" } },
      }),
    ),
  ).toEqual({ id: "a/b" });
  expect(testRequest(fetch.mock.calls[1][0]).url).toContain(
    "/api/conversations/a%2Fb",
  );
});

it.each([
  [401, '{"error":"unauthorized","login":"/auth/login"}', "请先登录后再继续。"],
  [403, "Origin rejected", "请求来源未被允许，请检查开发环境配置。"],
  [404, "null", "找不到这段会话。"],
  [409, "{}", "导入请求冲突，请重新打开导入窗口。"],
  [
    409,
    '{"error":"session_already_exists"}',
    "该来源已有相同的 Session ID，请检查来源或对应会话。",
  ],
  [413, "body too large", "文件或消息超过大小限制。"],
  [
    400,
    '{"kind":"invalid","path":"history[0].role","message":"invalid role"}',
    "history[0].role：invalid role",
  ],
  [502, "<html>proxy failure</html>", "服务暂时不可用，请稍后重试。"],
])("preserves localized HTTP %i errors", async (status, body, message) => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(
    new Response(body, { status }),
  );
  await expect(libraryOptions.queryFn()).rejects.toEqual(
    new ApiError(status, message),
  );
});

it("propagates network failures and rejects empty successful responses", async () => {
  const failure = new Error("网络中断");
  const fetch = vi.spyOn(globalThis, "fetch").mockRejectedValueOnce(failure);
  await expect(libraryOptions.queryFn()).rejects.toBe(failure);
  fetch.mockResolvedValueOnce(new Response(""));
  await expect(libraryOptions.queryFn()).rejects.toEqual(
    new ApiError(200, "服务暂时不可用，请稍后重试。"),
  );
});

it("serializes all import parts with an automatically generated multipart boundary", async () => {
  const result: components["schemas"]["ImportResult"] = {
    import_id: "import",
    conversation_id: "conversation",
    path_id: "path",
    head_message_id: "head",
    created: 1,
    reused: 0,
  };
  const fetch = vi
    .spyOn(globalThis, "fetch")
    .mockResolvedValue(Response.json(result));
  const history = '[{"role":"user","content":"你好"}]';
  const body: components["schemas"]["FileImport"] = {
    history: new File([history], "chat.json", { type: "application/json" }),
    title: "标题",
    source: "chatgpt",
    session_id: "session",
    occurred_at: "1789648800000",
    idempotency_key: "key",
  };
  expect(
    await apiData(
      api.POST("/api/import/file", {
        body,
        bodySerializer: serializeImport,
      }),
    ),
  ).toEqual(result);
  const request = testRequest(fetch.mock.calls[0][0]);
  expect(request.headers.get("Content-Type")).toMatch(
    /^multipart\/form-data; boundary=.+/,
  );
  expect(request.headers.has("Origin")).toBe(false);
  const form = await request.formData();
  const file = form.get("history") as File;
  expect([file.name, file.type, await file.text()]).toEqual([
    "chat.json",
    "application/json",
    history,
  ]);
  form.delete("history");
  const { history: _history, ...fields } = body;
  expect(Object.fromEntries(form)).toEqual(fields);
  expect(_history).toBeInstanceOf(File);
});

it("keeps browser bodies, nullable fields and cursors tied to generated types", () => {
  expectTypeOf<
    components["schemas"]["FileImport"]["history"]
  >().toEqualTypeOf<Blob>();
  expectTypeOf<components["schemas"]["Cursor"]>().toEqualTypeOf<string>();
  expectTypeOf<
    components["schemas"]["Message"]["parent_message_id"]
  >().toEqualTypeOf<string | null>();
  // This function is compiled but never called: negative cases must fail TypeScript, not send requests.
  const compileOnly = () => {
    // @ts-expect-error Unknown API paths must not compile.
    api.GET("/api/unknown");
    // @ts-expect-error The list endpoint has no PUT operation.
    api.PUT("/api/conversations", { body: {} });
    // @ts-expect-error The detail operation requires its id.
    api.GET("/api/conversations/{id}");
    api.PUT("/api/conversations/{id}", {
      params: { path: { id: "id" } },
      // @ts-expect-error Unknown source values must not compile.
      body: { title: "title", source: "unknown" },
    });
    api.PUT("/api/conversations/{id}/paths/{path_id}", {
      params: { path: { id: "id", path_id: "path" } },
      // @ts-expect-error JSON timestamps are numbers, unlike multipart timestamps.
      body: { history: "[]", occurred_at: "123", idempotency_key: "key" },
    });
  };
  expectTypeOf(compileOnly).toBeFunction();
});
