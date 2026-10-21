import { describe, expect, it } from "vitest";
import { createHandler, type Settings, type SettingsStore } from "./palace";
import type { Request } from "./protocol";

const settings: Settings = {
  server: "https://palace.test",
  token: "palace_pat_secret",
};

function memory(initial: Settings | null) {
  let stored = initial;
  const store: SettingsStore = {
    read: async () => stored,
    write: async (next) => {
      stored = next;
    },
  };
  return { store, stored: () => stored };
}

// A Palace that answers by method and path, and records what it was asked.
function palace(routes: Record<string, (request: Request) => Response>) {
  const seen: globalThis.Request[] = [];
  const fetcher = (async (input: RequestInfo | URL) => {
    const request = input as globalThis.Request;
    seen.push(request);
    const url = new URL(request.url);
    const route = routes[`${request.method} ${url.pathname}`];
    if (!route)
      return Response.json({ error: "not_found", login: "" }, { status: 404 });
    return route(request as never);
  }) as typeof fetch;
  return { fetcher, seen };
}

const json = (body: unknown, status = 200) => Response.json(body, { status });

const webAnnotation = {
  id: "w1",
  page_id: "p1",
  quote: "引文",
  prefix: "",
  suffix: "",
  position: 0,
  style: "yellow",
  note: null,
  created_at: 1,
  updated_at: 1,
};

// Core test case:
// - `specs/test-cases/extension/web-annotations.md#the-access-token-must-never-reach-the-page-context`
describe("the service worker", () => {
  it("refuses a content script anything beyond its own page", async () => {
    const { fetcher, seen } = palace({});
    const handle = createHandler(memory(settings).store, fetcher);
    for (const request of [
      { kind: "settings" },
      { kind: "save-settings", server: "https://evil.test", token: "x" },
      { kind: "pages" },
      { kind: "candidates", near: "https://a.test/", q: "", cursor: null },
      { kind: "bind", url: "https://a.test/", title: "", article_id: "a1" },
    ] satisfies Request[])
      expect(await handle(request, "content")).toEqual({
        ok: false,
        problem: "invalid",
        message: "Palace 没有接受这次修改。",
      });
    expect(seen).toEqual([]);
  });

  it("never hands the token back, and keeps it only once Palace accepts it", async () => {
    const memoryStore = memory(null);
    const { fetcher, seen } = palace({
      "GET /api/me": (request) =>
        (request as unknown as globalThis.Request).headers.get(
          "Authorization",
        ) === "Bearer palace_pat_good"
          ? json({})
          : json(
              { error: "authentication_required", login: "/auth/login" },
              401,
            ),
    });
    const handle = createHandler(memoryStore.store, fetcher);
    expect(
      await handle(
        {
          kind: "save-settings",
          server: "https://palace.test/",
          token: "palace_pat_bad",
        },
        "extension",
      ),
    ).toEqual({
      ok: false,
      problem: "unauthorized",
      message: "Access Token 无效，请在设置里换一个。",
    });
    expect(memoryStore.stored()).toBeNull();
    expect(
      await handle(
        {
          kind: "save-settings",
          server: "https://palace.test/",
          token: " palace_pat_good ",
        },
        "extension",
      ),
    ).toEqual({
      ok: true,
      data: { server: "https://palace.test", configured: true },
    });
    // A blank token keeps the stored one while the address changes.
    expect(
      await handle(
        { kind: "save-settings", server: "https://other.test/", token: "" },
        "extension",
      ),
    ).toEqual({
      ok: true,
      data: { server: "https://other.test", configured: true },
    });
    expect(memoryStore.stored()).toEqual({
      server: "https://other.test",
      token: "palace_pat_good",
    });
    expect(await handle({ kind: "settings" }, "extension")).toEqual({
      ok: true,
      data: { server: "https://other.test", configured: true },
    });
    // Every request names the token itself and never sends a cookie.
    expect(seen.map((request) => request.credentials)).toEqual([
      "omit",
      "omit",
      "omit",
    ]);
  });

  it("reads an unannotated page as unbound and a bound one as its article", async () => {
    const { fetcher } = palace({
      "GET /api/web-pages/lookup": (request) =>
        new URL(
          (request as unknown as globalThis.Request).url,
        ).searchParams.get("url") === "https://a.test/bound"
          ? json({
              page: {
                id: "p1",
                url: "https://a.test/bound",
                title: "t",
                article_id: "a1",
                updated_at: 1,
              },
              annotations: [webAnnotation],
            })
          : json({ error: "not_found", login: "" }, 404),
      "GET /api/articles/a1": () =>
        json({ id: "a1", title: "文章", content: "正文😀" }),
      "GET /api/articles/a1/annotations": () => json({ items: [] }),
    });
    const handle = createHandler(memory(settings).store, fetcher);
    expect(
      await handle({ kind: "page", url: "https://a.test/new" }, "content"),
    ).toEqual({
      ok: true,
      data: { kind: "unbound", annotations: [] },
    });
    expect(
      await handle({ kind: "page", url: "https://a.test/bound" }, "content"),
    ).toEqual({
      ok: true,
      data: {
        kind: "bound",
        article: {
          id: "a1",
          title: "文章",
          link: "https://palace.test/articles/a1",
          length: 4,
        },
        annotations: [],
      },
    });
  });

  it("names what went wrong", async () => {
    const annotation = {
      url: "https://a.test/",
      title: "",
      quote: "q",
      prefix: "",
      suffix: "",
      position: 0,
      style: "blue",
      note: null,
    } as const;
    const create: Request = { kind: "create", annotation };
    const cases = [
      [() => json({ error: "web_page_bound", login: "" }, 409), "bound"],
      [
        () => json({ error: "authentication_required", login: "" }, 401),
        "unauthorized",
      ],
      [
        () => json({ kind: "field", path: "quote", message: "" }, 400),
        "invalid",
      ],
      [() => json({ error: "persistence_failed", login: "" }, 500), "offline"],
      [
        () => {
          throw new TypeError("Failed to fetch");
        },
        "offline",
      ],
    ] as const;
    for (const [answer, problem] of cases) {
      const { fetcher } = palace({ "POST /api/web-pages/annotations": answer });
      const reply = await createHandler(memory(settings).store, fetcher)(
        create,
        "content",
      );
      expect(reply.ok ? null : reply.problem).toBe(problem);
    }
    const { fetcher, seen } = palace({});
    expect(
      await createHandler(memory(null).store, fetcher)(create, "content"),
    ).toEqual({
      ok: false,
      problem: "unconfigured",
      message: "还没有设置 Palace 地址和 Access Token。",
    });
    expect(seen).toEqual([]);
  });

  it("asks for candidates near the page and binds through Palace", async () => {
    const { fetcher, seen } = palace({
      "GET /api/articles": () =>
        json({ items: [], next_cursor: null, total: 0 }),
      "PUT /api/web-pages/binding": () =>
        json({
          id: "p1",
          url: "https://a.test/x",
          title: "t",
          article_id: "a1",
          updated_at: 1,
        }),
      "GET /api/web-pages/lookup": () =>
        json({
          page: {
            id: "p1",
            url: "https://a.test/x",
            title: "t",
            article_id: "a1",
            updated_at: 1,
          },
          annotations: [],
        }),
      "GET /api/articles/a1": () =>
        json({ id: "a1", title: "文章", content: "正文" }),
      "GET /api/articles/a1/annotations": () => json({ items: [] }),
    });
    const handle = createHandler(memory(settings).store, fetcher);
    await handle(
      {
        kind: "candidates",
        near: "https://a.test/x?y=1",
        q: " ",
        cursor: null,
      },
      "extension",
    );
    expect(new URL(seen[0].url).search).toBe(
      "?near=https%3A%2F%2Fa.test%2Fx%3Fy%3D1",
    );
    const bound = await handle(
      { kind: "bind", url: "https://a.test/x", title: "t", article_id: "a1" },
      "extension",
    );
    expect(bound.ok && bound.data.kind).toBe("bound");
    expect(await seen[1].json()).toEqual({
      url: "https://a.test/x",
      title: "t",
      article_id: "a1",
    });
    expect(
      seen.every(
        (request) =>
          request.headers.get("Authorization") === "Bearer palace_pat_secret",
      ),
    ).toBe(true);
  });
});
