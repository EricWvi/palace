import {
  chromium,
  expect,
  test,
  type BrowserContext,
  type Page,
  type Worker,
} from "@playwright/test";
import { createServer, type Server } from "node:http";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const dist = fileURLToPath(new URL("../dist", import.meta.url));
const token = "palace_pat_e2e-secret-token";

const article = `<!doctype html><html><head><title>原网页标题</title></head><body>
  <nav>导航 · 首页 · 关于</nav>
  <article>
    <p>收藏夹里躺着的文章，大多数再也没有被打开过。</p>
    <p>划线和旁批不一样：它们发生在阅读的当下。</p>
  </article>
  <script>
    // Everything the page itself can observe, for the test to search for the token.
    window.seen = [];
    addEventListener("message", (event) => window.seen.push(JSON.stringify(event.data)));
  </script>
</body></html>`;

type Body = Record<string, unknown>;

// One process serves the web page and plays Palace, keeping state in memory as Palace would.
function palace() {
  const requests: {
    method: string;
    path: string;
    body: Body | null;
    auth: string | null;
  }[] = [];
  const web: Body[] = [];
  let bound: string | null = null;
  let tokenValid = true;
  const server: Server = createServer(async (request, response) => {
    const url = new URL(request.url!, "http://localhost");
    let text = "";
    for await (const chunk of request) text += chunk;
    const body = text ? (JSON.parse(text) as Body) : null;
    if (url.pathname.startsWith("/api/"))
      requests.push({
        method: request.method!,
        path: url.pathname,
        body,
        auth: request.headers.authorization ?? null,
      });
    const send = (status: number, value?: unknown) => {
      response.writeHead(status, { "Content-Type": "application/json" });
      response.end(value === undefined ? "" : JSON.stringify(value));
    };
    if (url.pathname === "/article") {
      response.writeHead(200, { "Content-Type": "text/html; charset=utf-8" });
      return response.end(article);
    }
    if (url.pathname === "/dark") {
      response.writeHead(200, { "Content-Type": "text/html; charset=utf-8" });
      return response.end(
        article.replace(
          "<body>",
          '<body style="background: #1f1e1c; color: #e6e6e6">',
        ),
      );
    }
    if (url.pathname === "/articles/a1") {
      response.writeHead(200, { "Content-Type": "text/html; charset=utf-8" });
      return response.end("<title>Palace 文章页</title>");
    }
    if (url.pathname.startsWith("/api/")) {
      if (!tokenValid || request.headers.authorization !== `Bearer ${token}`)
        return send(401, {
          error: "authentication_required",
          login: "/auth/login",
        });
    }
    const page = (address: string) => ({
      id: "p1",
      url: address,
      title: "原网页标题",
      article_id: bound,
      updated_at: 1,
    });
    switch (`${request.method} ${url.pathname}`) {
      case "GET /api/me":
        return send(200, { id: "o1" });
      case "GET /api/web-pages/lookup":
        if (web.length === 0 && bound === null)
          return send(404, { error: "not_found", login: "" });
        return send(200, {
          page: page(url.searchParams.get("url")!),
          annotations: bound ? [] : web,
        });
      case "POST /api/web-pages/annotations": {
        if (bound) return send(409, { error: "web_page_bound", login: "" });
        const created: Body = {
          id: `w${web.length + 1}`,
          page_id: "p1",
          created_at: 1,
          updated_at: 1,
          ...body,
        };
        // The page's address and title belong to the page, not the annotation.
        delete created.url;
        delete created.title;
        web.push(created);
        return send(201, created);
      }
      case "GET /api/articles":
        return send(200, {
          items: [
            {
              id: "a1",
              title: "把一篇长文读慢一点",
              author: "林间",
              url: "https://example.com/slow",
              site: "example.com",
              published_at: 1,
              date: "2024-05-03",
            },
          ],
          next_cursor: null,
          total: 1,
        });
      case "PUT /api/web-pages/binding":
        bound = body!.article_id as string;
        return send(200, page(body!.url as string));
      case "GET /api/articles/a1":
        return send(200, {
          id: "a1",
          title: "把一篇长文读慢一点",
          content:
            "收藏夹里躺着的文章，大多数再也没有被打开过。\n\n划线和旁批不一样：它们发生在阅读的当下。",
        });
      case "GET /api/articles/a1/annotations":
        return send(200, {
          items: [
            {
              id: "n1",
              message_id: null,
              start: 25,
              end: 30,
              quote: "划线和旁批",
              style: "wavy",
              note: "迁移过来的批注",
              created_at: 1,
              updated_at: 1,
            },
          ],
        });
      case "GET /api/web-pages":
        return send(200, { items: [] });
    }
    send(404, { error: "not_found", login: "" });
  });
  return {
    server,
    requests,
    web,
    revoke: () => (tokenValid = false),
    origin: async () => {
      await new Promise<void>((resolve) =>
        server.listen(0, "127.0.0.1", resolve),
      );
      const address = server.address() as { port: number };
      return `http://127.0.0.1:${address.port}`;
    },
  };
}

let context: BrowserContext;
let worker: Worker;
let profile: string;
let fake: ReturnType<typeof palace>;
let origin: string;
let extension: string;

test.beforeEach(async () => {
  fake = palace();
  origin = await fake.origin();
  profile = await mkdtemp(join(tmpdir(), "palace-extension-"));
  context = await chromium.launchPersistentContext(profile, {
    channel: "chromium",
    args: [`--disable-extensions-except=${dist}`, `--load-extension=${dist}`],
  });
  worker =
    context.serviceWorkers()[0] ??
    (await context.waitForEvent("serviceworker"));
  extension = new URL(worker.url()).host;
  // Settings go in through the extension page, as the reader enters them.
  const settings = await context.newPage();
  await settings.goto(`chrome-extension://${extension}/pages.html`);
  await settings.getByLabel("Palace 地址").fill(origin);
  await settings.getByLabel("Access Token").fill(token);
  await settings.getByRole("button", { name: "保存" }).click();
  await expect(settings.getByText("已保存。")).toBeVisible();
  await settings.close();
});

test.afterEach(async () => {
  await context.close();
  fake.server.close();
  await rm(profile, { recursive: true, force: true });
});

// Selects the first occurrence of `quote` and lets go of the mouse, as a drag would end.
async function select(page: Page, quote: string) {
  await page.evaluate((quote) => {
    const walker = document.createTreeWalker(
      document.body,
      NodeFilter.SHOW_TEXT,
    );
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      const at = (node as Text).data.indexOf(quote);
      if (at === -1) continue;
      const range = document.createRange();
      range.setStart(node, at);
      range.setEnd(node, at + quote.length);
      getSelection()!.removeAllRanges();
      getSelection()!.addRange(range);
      document.body.dispatchEvent(new MouseEvent("mouseup", { bubbles: true }));
      return;
    }
  }, quote);
  // The extension reads the selection in a task queued on mouseup; one more task runs after it.
  await page.evaluate(() => new Promise((resolve) => setTimeout(resolve)));
}

// The words each Palace highlight draws on the page.
const highlights = (page: Page) =>
  page.evaluate(() =>
    Object.fromEntries(
      [...CSS.highlights.entries()].map(([name, set]) => [
        name,
        [...set].map((range) => range.toString()),
      ]),
    ),
  );

const tabOf = (url: string) =>
  worker.evaluate(
    async (url) => (await chrome.tabs.query({ url }))[0].id!,
    url,
  );

// Core test case:
// - `specs/test-cases/extension/web-annotations.md#the-access-token-must-never-reach-the-page-context`
test("annotating, binding and the read-only display never put the token in the page", async () => {
  const page = await context.newPage();
  await page.goto(`${origin}/article`);
  await expect
    .poll(() =>
      fake.requests.some((request) => request.path === "/api/web-pages/lookup"),
    )
    .toBe(true);

  // Annotate with the bar's key: the page is unbound, so the extension writes to Palace.
  await select(page, "大多数再也没有被打开过");
  await page.screenshot({ path: "test-results/extension-bar.png" });
  await page.keyboard.press("1");
  await expect.poll(() => fake.web.length).toBe(1);
  expect(fake.web[0]).toMatchObject({
    quote: "大多数再也没有被打开过",
    prefix: expect.stringMatching(/收藏夹里躺着的文章，$/),
    suffix: expect.stringMatching(/^。/),
    style: "yellow",
    note: null,
  });
  await expect
    .poll(() => highlights(page))
    .toEqual({
      "palace-yellow-light": ["大多数再也没有被打开过"],
    });

  // Bind through the popup, opened for the page's tab.
  const tab = await tabOf(`${origin}/article`);
  const popup = await context.newPage();
  await popup.addInitScript(
    ({ tab, url }) => {
      chrome.tabs.query = (async () => [
        { id: tab, url, title: "原网页标题" },
      ]) as never;
    },
    { tab, url: `${origin}/article` },
  );
  await popup.goto(`chrome-extension://${extension}/popup.html`);
  await expect(popup.getByText("这个网页有 1 处标注。")).toBeVisible();
  await popup.getByRole("button", { name: "收进 Palace" }).click();
  await popup.setViewportSize({ width: 360, height: 240 });
  await expect(
    popup.getByRole("button", { name: /把一篇长文读慢一点/ }),
  ).toBeVisible();
  await popup.screenshot({ path: "test-results/extension-popup.png" });
  const opened = context.waitForEvent("page");
  await popup.getByRole("button", { name: /把一篇长文读慢一点/ }).click();
  expect((await opened).url()).toBe(`${origin}/articles/a1`);
  expect(
    fake.requests.find((request) => request.path === "/api/web-pages/binding")
      ?.body,
  ).toEqual({
    url: `${origin}/article`,
    title: "原网页标题",
    article_id: "a1",
  });

  // The bound page shows the article's annotations and takes no new ones.
  await expect
    .poll(() => highlights(page))
    .toEqual({ "palace-wavy-light": ["划线和旁批"] });
  // Hovering the noted mark shows its note, read only.
  const mark = await page.evaluate(() => {
    const range = [...CSS.highlights.get("palace-wavy-light")!][0] as Range;
    const box = range.getBoundingClientRect();
    return { x: box.left + 4, y: box.top + box.height / 2 };
  });
  await page.mouse.move(mark.x, mark.y);
  await page.screenshot({ path: "test-results/extension-bound.png" });
  const posted = fake.requests.filter(
    (request) => request.method === "POST",
  ).length;
  await select(page, "收藏夹");
  await page.keyboard.press("2");
  expect(
    fake.requests.filter((request) => request.method === "POST").length,
  ).toBe(posted);
  expect(
    await worker.evaluate(
      (tab) => chrome.action.getBadgeText({ tabId: tab }),
      tab,
    ),
  ).toBe("✓");

  // Nothing the page can reach holds the token; the extension never syncs it.
  const exposed = await page.evaluate((token) => {
    const globals = Object.getOwnPropertyNames(window)
      .map((name) => {
        try {
          return String((window as unknown as Record<string, unknown>)[name]);
        } catch {
          return "";
        }
      })
      .join("\n");
    const seen = (window as unknown as { seen: string[] }).seen.join("\n");
    const stored =
      JSON.stringify({ ...localStorage }) +
      JSON.stringify({ ...sessionStorage });
    return [
      document.documentElement.outerHTML,
      globals,
      seen,
      stored,
      document.cookie,
    ].some((text) => text.includes(token));
  }, token);
  expect(exposed).toBe(false);
  expect(await worker.evaluate(() => chrome.storage.sync.get(null))).toEqual(
    {},
  );
  expect(
    fake.requests.every((request) => request.auth === `Bearer ${token}`),
  ).toBe(true);
});

// A site with its own dark ground gets Palace's dark marks, whatever the browser's appearance.
test("marks on a dark page take the dark palette", async () => {
  const page = await context.newPage();
  await page.goto(`${origin}/dark`);
  await expect
    .poll(() =>
      fake.requests.some((request) => request.path === "/api/web-pages/lookup"),
    )
    .toBe(true);
  await select(page, "大多数再也没有被打开过");
  await page.keyboard.press("1");
  await expect
    .poll(() => highlights(page))
    .toEqual({ "palace-yellow-dark": ["大多数再也没有被打开过"] });
});

// Core test case:
// - `specs/test-cases/extension/web-annotations.md#the-access-token-must-never-reach-the-page-context`
test("an invalid token turns the bar off and marks the toolbar icon", async () => {
  fake.revoke();
  const page = await context.newPage();
  await page.goto(`${origin}/article`);
  const tab = await tabOf(`${origin}/article`);
  await expect
    .poll(() =>
      worker.evaluate((tab) => chrome.action.getTitle({ tabId: tab }), tab),
    )
    .toBe("Access Token 无效");
  await select(page, "大多数");
  await page.keyboard.press("1");
  expect(fake.requests.filter((request) => request.method === "POST")).toEqual(
    [],
  );
  expect(
    await worker.evaluate(
      (tab) => chrome.action.getBadgeText({ tabId: tab }),
      tab,
    ),
  ).toBe("!");
});
