// The service worker's side of every request: the only code that holds the access token and
// calls Palace. The browser's storage and fetch are passed in, so tests run it against a fake
// server without a browser.
import createClient from "openapi-fetch";
import type { paths } from "../../palace-web/src/lib/generated/api";
import {
  contentKinds,
  type Answer,
  type Kind,
  type PageState,
  type Problem,
  type Reply,
  type Request,
} from "./protocol";

export type Settings = { server: string; token: string };

// Where the address and token live: `chrome.storage.local` in the browser, never `sync`, so a
// token stays on the computer it was made for.
export interface SettingsStore {
  read(): Promise<Settings | null>;
  write(settings: Settings): Promise<void>;
}

// Who sent a request: a content script inside a web page, or a page of the extension itself.
export type Sender = "content" | "extension";

type Client = ReturnType<typeof createClient<paths>>;

class Failure extends Error {
  constructor(
    public problem: Problem,
    message: string,
  ) {
    super(message);
  }
}

const problems: Record<Problem, string> = {
  unconfigured: "还没有设置 Palace 地址和 Access Token。",
  offline: "连不上 Palace。",
  unauthorized: "Access Token 无效，请在设置里换一个。",
  bound: "这个网页已经收进 Palace，请在文章页里标注。",
  missing: "Palace 里没有这一项了。",
  invalid: "Palace 没有接受这次修改。",
};

// Unwraps a response, turning each failure into the problem the pages act on.
async function data<T>(
  operation: Promise<{ data?: T; error?: unknown; response: Response }>,
): Promise<T> {
  let result: { data?: T; error?: unknown; response: Response };
  try {
    result = await operation;
  } catch {
    throw new Failure("offline", problems.offline);
  }
  const { response, error } = result;
  if (response.ok) return result.data as T;
  const code =
    typeof error === "object" && error !== null && "error" in error
      ? error.error
      : null;
  if (response.status === 401)
    throw new Failure("unauthorized", problems.unauthorized);
  if (response.status === 409 && code === "web_page_bound")
    throw new Failure("bound", problems.bound);
  if (response.status === 404) throw new Failure("missing", problems.missing);
  if (response.status === 400 || response.status === 413)
    throw new Failure("invalid", problems.invalid);
  throw new Failure("offline", problems.offline);
}

// Accepts an absolute http(s) address and drops any trailing slash, so paths join cleanly.
function serverAddress(text: string): string | null {
  try {
    const url = new URL(text.trim());
    if (url.protocol !== "http:" && url.protocol !== "https:") return null;
    return url.origin + url.pathname.replace(/\/+$/, "");
  } catch {
    return null;
  }
}

// Builds the handler for one service worker: every request reads the settings afresh, so a
// change in the extension page applies to the next request without a restart.
export function createHandler(store: SettingsStore, fetcher: typeof fetch) {
  const client = (settings: Settings): Client =>
    createClient<paths>({
      baseUrl: settings.server,
      // A session cookie for the same server must never stand in for the token.
      credentials: "omit",
      headers: { Authorization: `Bearer ${settings.token}` },
      fetch: (request) => fetcher(request),
    });

  // What the page at `url` is: its own annotations, or the article it is bound to.
  async function page(settings: Settings, url: string): Promise<PageState> {
    const api = client(settings);
    let found;
    try {
      found = await data(
        api.GET("/api/web-pages/lookup", { params: { query: { url } } }),
      );
    } catch (error) {
      // A page never annotated has no record yet.
      if (error instanceof Failure && error.problem === "missing")
        return { kind: "unbound", annotations: [] };
      throw error;
    }
    const article = found.page.article_id;
    if (article === null)
      return { kind: "unbound", annotations: found.annotations };
    const path = { params: { path: { id: article } } };
    const [detail, annotations] = await Promise.all([
      data(api.GET("/api/articles/{id}", path)),
      data(api.GET("/api/articles/{id}/annotations", path)),
    ]);
    return {
      kind: "bound",
      article: {
        id: detail.id,
        title: detail.title,
        link: `${settings.server}/articles/${detail.id}`,
        length: detail.content.length,
      },
      annotations: annotations.items,
    };
  }

  async function answer<K extends Kind>(
    request: Request<K>,
    settings: Settings | null,
  ): Promise<Answer<K>>;
  async function answer(
    request: Request,
    settings: Settings | null,
  ): Promise<unknown> {
    switch (request.kind) {
      case "settings":
        return {
          server: settings?.server ?? "",
          configured: settings !== null,
        };
      case "save-settings": {
        const server = serverAddress(request.server);
        if (server === null)
          throw new Failure(
            "invalid",
            "Palace 地址需要以 http:// 或 https:// 开头。",
          );
        // A blank token field keeps the stored token, so the address can change on its own.
        const next = {
          server,
          token: request.token.trim() || settings?.token || "",
        };
        // A token is kept only once Palace accepts it.
        await data(client(next).GET("/api/me"));
        await store.write(next);
        return { server, configured: true };
      }
    }
    if (settings === null)
      throw new Failure("unconfigured", problems.unconfigured);
    const api = client(settings);
    switch (request.kind) {
      case "page":
        return page(settings, request.url);
      case "create":
        return data(
          api.POST("/api/web-pages/annotations", { body: request.annotation }),
        );
      case "update":
        return data(
          api.PUT("/api/web-annotations/{id}", {
            params: { path: { id: request.id } },
            body: request.change,
          }),
        );
      case "delete":
        await data(
          api.DELETE("/api/web-annotations/{id}", {
            params: { path: { id: request.id } },
          }),
        );
        return null;
      case "candidates":
        return data(
          api.GET("/api/articles", {
            params: {
              query: {
                near: request.near,
                q: request.q.trim() || undefined,
                cursor: request.cursor ?? undefined,
              },
            },
          }),
        );
      case "bind":
        await data(
          api.PUT("/api/web-pages/binding", {
            body: {
              url: request.url,
              title: request.title,
              article_id: request.article_id,
            },
          }),
        );
        return page(settings, request.url);
      case "pages":
        return data(api.GET("/api/web-pages"));
    }
  }

  // Answers one request; a content script is refused anything beyond its own page.
  return async function handle<K extends Kind>(
    request: Request<K>,
    sender: Sender,
  ): Promise<Reply<Answer<K>>> {
    if (sender === "content" && !contentKinds.has(request.kind))
      return { ok: false, problem: "invalid", message: problems.invalid };
    try {
      const settings = await store.read();
      return { ok: true, data: await answer(request, settings) };
    } catch (error) {
      if (error instanceof Failure)
        return { ok: false, problem: error.problem, message: error.message };
      throw error;
    }
  };
}
