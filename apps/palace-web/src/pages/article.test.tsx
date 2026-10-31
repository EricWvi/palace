import { screen, waitFor, within } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { address, mountApp } from "@/test-app";
import type { Article, ArticleListItem, Moment } from "@/lib/api";
import { dayRange, parseDay } from "@/lib/day";
import { bareDay, requestPath, testRequest } from "@/lib/test-request";

// Local wall-clock times, so years and dates do not depend on the test machine's zone.
const at = (year: number, month: number, day: number) =>
  new Date(year, month - 1, day, 12).getTime();
const cover = "0192a3b4-0000-7000-8000-000000000001";
const demo = "0192a3b4-0000-7000-8000-000000000002";
const elsewhere = "0192a3b4-0000-7000-8000-000000000009";
const article: Article = {
  id: "a1",
  title: "元数据标题",
  content: [
    "# 文章 **标题**",
    "",
    `![封面](/api/media/${cover})`,
    "",
    `![演示](/api/media/${demo})`,
    "",
    "![原站](https://origin.example/pic.png) ![内嵌](data:image/png;base64,AAAA)",
    "",
    `![别处](/api/media/${elsewhere})`,
    "",
    "## 小节",
    "",
    "正文。",
  ].join("\n"),
  url: "https://www.example.com/post",
  author: "某人",
  site: "example.com",
  published_at: at(2024, 5, 1),
  date: "2024-05-01",
  occurred_at: at(2026, 10, 20),
  media: [
    { id: cover, content_type: "image/png" },
    { id: demo, content_type: "video/mp4" },
  ],
};
const listed: ArticleListItem[] = [
  {
    id: "a2",
    title: "无名作者",
    author: null,
    url: "https://blog.example.org/nameless",
    site: "blog.example.org",
    published_at: at(2026, 3, 9),
    date: "2026-03-09",
  },
  {
    id: "a1",
    title: article.title,
    author: article.author,
    url: article.url,
    site: article.site,
    published_at: article.published_at,
    date: article.date,
  },
];

// Serves one stored article and records what edits and deletions reach the server.
function serve({ conflict = false } = {}) {
  let stored: Article | null = article;
  const sent: { method: string; path: string; body: unknown }[] = [];
  vi.spyOn(globalThis, "fetch").mockImplementation(async (url) => {
    const request = testRequest(url);
    const path = requestPath(url);
    const bare = bareDay(path);
    if (bare) return bare;
    if (path === "/api/articles/a1/annotations")
      return Response.json({ items: [] });
    if (path === "/api/articles")
      return Response.json({
        items: stored ? listed : listed.slice(0, 1),
        next_cursor: null,
        total: stored ? 2 : 1,
      });
    if (path.startsWith("/api/articles/") && path !== "/api/articles/a1")
      return Response.json({ error: "not_found", login: "" }, { status: 404 });
    if (path === "/api/articles/a1") {
      if (request.method !== "GET") {
        const body = request.method === "PUT" ? await request.json() : null;
        sent.push({ method: request.method, path, body });
        if (request.method === "DELETE") {
          stored = null;
          return new Response(null, { status: 204 });
        }
        if (conflict)
          return Response.json(
            { error: "identity_or_request_conflict", login: "" },
            { status: 409 },
          );
        // A bare day is that day's midnight, as the server reads it in the deployment's zone.
        const [year, month, day] = (body.date as string).split("-").map(Number);
        stored = {
          ...stored!,
          ...body,
          author: body.author ?? null,
          published_at: new Date(year, month - 1, day).getTime(),
        };
      }
      return stored
        ? Response.json(stored)
        : Response.json({ error: "not_found", login: "" }, { status: 404 });
    }
    if (path === "/api/timeline/outline") return Response.json([]);
    if (path === "/api/timeline") return Response.json([]);
    return Response.json({ items: [], next_cursor: null, total: 0 });
  });
  return { sent };
}
const scrolled = vi.fn();
beforeEach(() => {
  // jsdom implements no scrolling; the article page asks for the top when opened from the list.
  window.scrollTo = scrolled as typeof window.scrollTo;
  scrolled.mockReset();
});

// Core test cases:
// - `specs/test-cases/web/stars/articles.md#article-page-must-load-only-its-own-palace-media`
// - `specs/test-cases/web/stars/articles.md#article-list-must-order-by-published-date-while-the-timeline-uses-import-time`
// - `specs/test-cases/web/stars/notes.md#stars-navigation-must-open-the-note-list-and-every-kind`
it("lists articles by publish date and reads one like a note, with its own media only", async () => {
  serve();
  const { router, user } = mountApp("/articles");
  await screen.findByText("无名作者");
  expect(
    [...document.querySelectorAll(".year")].map((year) => [
      year.querySelector("h2")!.textContent,
      [...year.querySelectorAll(".entry")].map((entry) => [
        entry.querySelector(".entry-title")!.textContent,
        entry.querySelector(".entry-aside")?.textContent ?? null,
        entry.querySelector(".entry-date")!.textContent,
        entry.getAttribute("href"),
      ]),
    ]),
  ).toEqual([
    ["2026", [["无名作者", "blog.example.org", "03.09", "/articles/a2"]]],
    ["2024", [["元数据标题", "某人", "05.01", "/articles/a1"]]],
  ]);

  await user.click(screen.getByRole("link", { name: /元数据标题/ }));
  await screen.findByText("正文。");
  expect(address(router)).toBe("/articles/a1");
  expect(scrolled).toHaveBeenCalledWith(0, 0);
  expect(document.title).toBe("元数据标题");
  // The body's leading heading stands in for the title, as on a note.
  expect(
    [...document.querySelectorAll("h1")].map((h1) => h1.innerHTML),
  ).toEqual(["文章 <strong>标题</strong>"]);
  const meta = document.querySelector("header.note .meta")!;
  expect([...meta.children].map((fact) => fact.textContent)).toEqual([
    "某人",
    "2024.05.01",
    "原文 ↗",
  ]);
  const original = within(meta as HTMLElement).getByRole("link", {
    name: "原文 ↗",
  });
  expect([
    original.getAttribute("href"),
    original.getAttribute("target"),
    original.getAttribute("rel"),
  ]).toEqual(["https://www.example.com/post", "_blank", "noreferrer"]);
  // Only the article's own files load, each as its type says; nothing else is fetched.
  expect(
    [...document.querySelectorAll(".note-body img")].map((img) =>
      img.getAttribute("src"),
    ),
  ).toEqual([`/api/media/${cover}`]);
  expect(
    [...document.querySelectorAll(".note-body video")].map((video) =>
      video.getAttribute("src"),
    ),
  ).toEqual([`/api/media/${demo}`]);
  expect(
    [...document.querySelectorAll(".note-body .quiet")].map(
      (placeholder) => placeholder.textContent,
    ),
  ).toEqual(["[图片：原站]", "[图片：内嵌]", "[图片：别处]"]);
  expect(
    within(screen.getByRole("navigation", { name: "文章目录" }))
      .getAllByRole("link")
      .map((link) => link.textContent),
  ).toEqual(["文章 标题", "小节"]);
  expect(
    document.querySelector('nav[aria-label="主导航"] [aria-current]'),
  ).toHaveTextContent("摘星");
  // From 摘星 there is no way back but the browser's.
  expect(screen.queryByRole("link", { name: /← / })).toBeNull();
});

// Core test case: `specs/test-cases/web/stars/articles.md#article-page-must-load-only-its-own-palace-media`
it("says when an article does not exist", async () => {
  serve();
  mountApp("/articles/gone");
  expect(await screen.findByRole("alert")).toHaveTextContent(
    "找不到这篇文章。",
  );
});

// Core test case: `specs/test-cases/web/stars/articles.md#editing-an-article-must-replace-only-its-metadata`
it("edits everything but the body and shows the new facts at once", async () => {
  const { sent } = serve();
  const { user } = mountApp("/articles/a1");
  await screen.findByText("正文。");
  screen.getByRole("button", { name: "管理文章" }).focus();
  await user.keyboard("{Enter}");
  await user.click(await screen.findByRole("menuitem", { name: "编辑文章" }));
  const dialog = await screen.findByRole("dialog", { name: "编辑文章" });
  const title = within(dialog).getByLabelText("标题");
  const author = within(dialog).getByLabelText("作者");
  const date = within(dialog).getByLabelText("发布日期");
  expect(
    [title, within(dialog).getByLabelText("原文地址"), author, date].map(
      (field) => (field as HTMLInputElement).value,
    ),
  ).toEqual([
    "元数据标题",
    "https://www.example.com/post",
    "某人",
    "2024-05-01",
  ]);
  await user.clear(author);
  await user.clear(date);
  await user.type(date, "2024-05-02");
  await user.click(within(dialog).getByRole("button", { name: "保存" }));
  await waitFor(() =>
    expect(screen.queryByRole("dialog", { name: "编辑文章" })).toBeNull(),
  );
  expect(sent).toEqual([
    {
      method: "PUT",
      path: "/api/articles/a1",
      body: {
        title: "元数据标题",
        url: "https://www.example.com/post",
        author: null,
        date: "2024-05-02",
      },
    },
  ]);
  const meta = document.querySelector("header.note .meta")!;
  await waitFor(() =>
    expect([...meta.children].map((fact) => fact.textContent)).toEqual([
      "2024.05.02",
      "原文 ↗",
    ]),
  );
});

// Core test case: `specs/test-cases/web/stars/articles.md#editing-an-article-must-replace-only-its-metadata`
it("keeps the dialog and the input when the address belongs to another article", async () => {
  serve({ conflict: true });
  const { user } = mountApp("/articles/a1");
  await screen.findByText("正文。");
  screen.getByRole("button", { name: "管理文章" }).focus();
  await user.keyboard("{Enter}");
  await user.click(await screen.findByRole("menuitem", { name: "编辑文章" }));
  const dialog = await screen.findByRole("dialog", { name: "编辑文章" });
  const url = within(dialog).getByLabelText("原文地址");
  await user.clear(url);
  await user.type(url, "https://example.com/taken");
  await user.click(within(dialog).getByRole("button", { name: "保存" }));
  expect(await within(dialog).findByRole("alert")).toHaveTextContent(
    "另一篇文章已经使用这个原文地址。",
  );
  expect(url).toHaveValue("https://example.com/taken");
  expect(
    document.querySelector("header.note .meta a")!.getAttribute("href"),
  ).toBe("https://www.example.com/post");
});

// Core test case: `specs/test-cases/web/stars/articles.md#deleting-an-article-must-remove-its-media-objects-before-reporting-success`
it("deletes the article and returns to the list without it", async () => {
  const { sent } = serve();
  const { router, user } = mountApp("/articles/a1");
  await screen.findByText("正文。");
  screen.getByRole("button", { name: "管理文章" }).focus();
  await user.keyboard("{Enter}");
  await user.click(await screen.findByRole("menuitem", { name: "删除文章" }));
  const dialog = await screen.findByRole("dialog", { name: "删除文章？" });
  expect(dialog).toHaveTextContent(
    "文章和其中的图片、视频都会被删除，无法撤销。",
  );
  await user.click(within(dialog).getByRole("button", { name: "删除文章" }));
  await waitFor(() => expect(address(router)).toBe("/articles"));
  expect(sent.map(({ method }) => method)).toEqual(["DELETE"]);
  await waitFor(() =>
    expect(
      [...document.querySelectorAll(".entry-title")].map((t) => t.textContent),
    ).toEqual(["无名作者"]),
  );
});

// Core test case: `specs/test-cases/web/stars/articles.md#article-list-must-order-by-published-date-while-the-timeline-uses-import-time`
it("shows an article on its import day by author or site, and leads back to that day", async () => {
  const day = parseDay("2026-10-20")!;
  const moments: Moment[] = [
    {
      kind: "article",
      id: "a1",
      occurred_at: day.getTime() + 1000,
      title: "元数据标题",
      author: "某人",
      site: "example.com",
    },
    {
      kind: "article",
      id: "a2",
      occurred_at: day.getTime() + 2000,
      title: "无名作者",
      author: null,
      site: "blog.example.org",
    },
  ];
  vi.spyOn(globalThis, "fetch").mockImplementation(async (url) => {
    const path = requestPath(url);
    const bare = bareDay(path);
    if (bare) return bare;
    const start = Number(
      new URL(testRequest(url).url).searchParams.get("start"),
    );
    const today = start === dayRange(day).start;
    if (path === "/api/timeline/outline")
      return Response.json(
        today ? moments.map(({ id, kind }) => ({ id, kind })) : [],
      );
    if (path === "/api/timeline") return Response.json(today ? moments : []);
    if (path === "/api/articles/a1") return Response.json(article);
    return Response.json({ items: [], next_cursor: null, total: 0 });
  });
  const { router, user } = mountApp("/?date=2026-10-20");
  await screen.findByText("无名作者");
  expect(
    [...document.querySelectorAll(".moment-head")].map((head) => [
      head.textContent,
      head.getAttribute("href"),
    ]),
  ).toEqual([
    ["文章元数据标题某人", "/articles/a1?date=2026-10-20"],
    ["文章无名作者blog.example.org", "/articles/a2?date=2026-10-20"],
  ]);
  await user.click(screen.getByRole("link", { name: /元数据标题/ }));
  await screen.findByText("正文。");
  const back = screen.getByRole("link", { name: /← / });
  expect(back).toHaveAttribute("href", "/?date=2026-10-20&moment=a1");
  await user.click(back);
  expect(address(router)).toBe("/?date=2026-10-20&moment=a1");
});
