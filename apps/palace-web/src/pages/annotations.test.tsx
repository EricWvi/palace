import {
  act,
  fireEvent,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { mountApp } from "@/test-app";
import type { Annotation, Article, Detail } from "@/lib/api";
import { scopeTexts } from "@/lib/annotation-dom";
import { requestPath, testRequest } from "@/lib/test-request";

const article: Article = {
  id: "a1",
  title: "慢读",
  content: [
    "第一段，有一句值得记住的话。",
    "",
    "第二段没有标注。",
    "",
    "第三段也没有。",
    "",
    "## 小节",
    "",
    "最后一段说到 `code` 的事。",
  ].join("\n"),
  url: "https://example.com/slow",
  author: null,
  site: "example.com",
  published_at: 1_700_000_000_000,
  date: "2023-11-15",
  occurred_at: 1_700_000_000_000,
  media: [],
};

const conversation: Detail = {
  conversation: { owner_id: "o", id: "c1", source: "chatgpt" },
  messages: ["问一句", "答一句", "再问", "再答"].map((content, index) => ({
    id: `m${index + 1}`,
    content,
    role: index % 2 ? "assistant" : "user",
    owner_id: "o",
    conversation_id: "c1",
    parent_message_id: index ? `m${index}` : null,
    toc_line: content,
    created_order: index,
  })),
  paths: [
    {
      id: "p1",
      title: "一段对话",
      session_id: "s1",
      head_message_id: "m4",
      message_count: 4,
      occurred_at: 1_700_000_000_000,
      created_at: 1000,
      updated_at: 1000,
      original_link: "https://chatgpt.com/c/s1",
    },
  ],
};

type Sent = { method: string; path: string; body: unknown };

// Serves one article and one conversation with annotations kept in memory, recording every
// change that reaches the server; `fail` makes every change fail.
function serve(initial: Annotation[] = [], { fail = false } = {}) {
  let stored = [...initial];
  let next = 1;
  const sent: Sent[] = [];
  vi.spyOn(globalThis, "fetch").mockImplementation(async (url) => {
    const request = testRequest(url);
    const path = requestPath(url);
    if (request.method !== "GET") {
      const body = request.method === "DELETE" ? null : await request.json();
      sent.push({ method: request.method, path, body });
      if (fail)
        return Response.json({ error: "internal", login: "" }, { status: 500 });
      if (request.method === "POST") {
        // A conversation's create carries the message; an article's has none.
        const fields = body as Omit<
          Annotation,
          "id" | "created_at" | "updated_at"
        >;
        const created: Annotation = {
          ...fields,
          id: `n${next++}`,
          message_id: fields.message_id ?? null,
          created_at: 1,
          updated_at: 1,
        };
        stored = [...stored, created];
        return Response.json(created, { status: 201 });
      }
      const id = path.split("/").at(-1)!;
      if (request.method === "PUT") {
        stored = stored.map((item) =>
          item.id === id ? { ...item, ...(body as object) } : item,
        );
        return Response.json(stored.find((item) => item.id === id));
      }
      stored = stored.filter((item) => item.id !== id);
      return new Response(null, { status: 204 });
    }
    if (path === "/api/articles/a1/annotations")
      return Response.json({
        items: stored.filter((item) => item.message_id === null),
      });
    if (path === "/api/conversations/c1/annotations")
      return Response.json({
        items: stored.filter((item) => item.message_id !== null),
      });
    if (path === "/api/articles/a1") return Response.json(article);
    if (path === "/api/conversations/c1") return Response.json(conversation);
    return Response.json([]);
  });
  return sent;
}

// The margin column needs a wide window; tests choose one.
function windowWide(wide: boolean) {
  vi.stubGlobal("matchMedia", () => ({
    matches: wide,
    addEventListener() {},
    removeEventListener() {},
  }));
}
afterEach(() => vi.unstubAllGlobals());

function annotation(
  quote: string,
  fields: Partial<Annotation> = {},
  text = article.content.replace("`code`", "code").replace("## ", ""),
): Annotation {
  const start = text.indexOf(quote);
  return {
    id: quote,
    message_id: null,
    start,
    end: start + quote.length,
    quote,
    style: "yellow",
    note: null,
    created_at: 1,
    updated_at: 1,
    ...fields,
  };
}

// Selects a quote inside one scope the way a drag would, then lets go of the mouse.
async function select(quote: string, scope = "article") {
  const root = document.querySelector(`[data-annotation-scope="${scope}"]`)!;
  const range = document.createRange();
  let position = 0;
  const texts = scopeTexts(root);
  const whole = texts.map((node) => node.data).join("");
  const start = whole.indexOf(quote);
  const end = start + quote.length;
  for (const text of texts) {
    const after = position + text.length;
    if (start >= position && start < after)
      range.setStart(text, start - position);
    if (end > position && end <= after) range.setEnd(text, end - position);
    position = after;
  }
  document.getSelection()!.removeAllRanges();
  document.getSelection()!.addRange(range);
  fireEvent.mouseUp(root);
  return screen.findByRole("toolbar", { name: "标注" });
}

const marks = () =>
  [...document.querySelectorAll<HTMLElement>("mark.anno")].map((mark) => [
    mark.dataset.annotation,
    mark.textContent,
  ]);

// Selecting text and choosing a style saves the anchor and draws it; a click on the mark
// offers the same bar, from which Delete removes it.
// Core test case:
// - `specs/test-cases/web/stars/annotations.md#marks-must-land-on-the-text-that-was-selected`
it("highlights a selection and removes it from its mark", async () => {
  windowWide(true);
  const sent = serve();
  const { user } = mountApp("/articles/a1");
  await screen.findByText(/第二段/);
  await select("值得记住的话");
  await user.keyboard("2");
  await waitFor(() => expect(marks()).toEqual([["n1", "值得记住的话"]]));
  expect(sent).toEqual([
    {
      method: "POST",
      path: "/api/articles/a1/annotations",
      body: {
        start: 7,
        end: 13,
        quote: "值得记住的话",
        style: "green",
        note: null,
      },
    },
  ]);
  // Highlights alone never widen the page.
  expect(document.querySelector(".margin-notes")).toBeNull();
  expect(screen.getByText("1 处标注")).toBeInTheDocument();

  await user.click(document.querySelector("mark.anno")!);
  const bar = await screen.findByRole("toolbar", { name: "标注" });
  expect(
    within(bar).getByRole("button", { name: "绿色高亮（2）" }),
  ).toHaveAttribute("aria-pressed", "true");
  await user.keyboard("{Delete}");
  await waitFor(() => expect(marks()).toEqual([]));
  expect(sent.at(-1)).toEqual({
    method: "DELETE",
    path: "/api/annotations/n1",
    body: null,
  });
});

// A note waits for words before it is saved, widens the page into a margin column while it
// exists, and an emptied note takes a note-only annotation away.
// Core test case:
// - `specs/test-cases/web/stars/annotations.md#margin-notes-must-widen-the-page-only-while-notes-exist`
it("writes a note in the margin and removes it when emptied", async () => {
  windowWide(true);
  const sent = serve([annotation("第三段", { style: "blue" })]);
  const { user } = mountApp("/articles/a1");
  await screen.findByText(/第二段/);
  expect(document.querySelector(".margin-notes")).toBeNull();
  await select("值得记住的话");
  await user.keyboard("c");
  const field = await screen.findByRole("textbox", { name: "批注" });
  expect(document.querySelector(".margin-notes")).not.toBeNull();
  expect(field).toHaveFocus();
  expect(sent).toEqual([]);
  await user.type(field, "这句可以当引语");
  await act(async () => field.blur());
  await waitFor(() =>
    expect(document.querySelector(".margin-notes")?.textContent).toBe(
      "这句可以当引语",
    ),
  );
  expect(sent).toEqual([
    {
      method: "POST",
      path: "/api/articles/a1/annotations",
      body: {
        start: 7,
        end: 13,
        quote: "值得记住的话",
        style: null,
        note: "这句可以当引语",
      },
    },
  ]);

  await user.click(screen.getByText("这句可以当引语"));
  const editing = await screen.findByRole("textbox", { name: "批注" });
  await user.clear(editing);
  await act(async () => editing.blur());
  await waitFor(() =>
    expect(document.querySelector(".margin-notes")).toBeNull(),
  );
  expect(sent.at(-1)).toEqual({
    method: "DELETE",
    path: "/api/annotations/n1",
    body: null,
  });
  expect(marks()).toEqual([["第三段", "第三段"]]);
});

// On a narrow window a note floats under its mark on hover instead of taking a margin.
// Core test case:
// - `specs/test-cases/web/stars/annotations.md#margin-notes-must-widen-the-page-only-while-notes-exist`
it("floats a note under its mark on a narrow window", async () => {
  windowWide(false);
  serve([annotation("值得记住的话", { style: null, note: "旁批" })]);
  const { user } = mountApp("/articles/a1");
  await screen.findByText(/第二段/);
  expect(document.querySelector(".margin-notes")).toBeNull();
  await user.hover(document.querySelector("mark.anno")!);
  const card = await screen.findByText("旁批");
  expect(card.closest(".note-card")).toHaveClass("floating");
});

// A failed save leaves the page as it was and says so.
// Core test case:
// - `specs/test-cases/web/stars/annotations.md#margin-notes-must-widen-the-page-only-while-notes-exist`
it("keeps the page unchanged when a save fails", async () => {
  windowWide(true);
  serve([], { fail: true });
  const { user } = mountApp("/articles/a1");
  await screen.findByText(/第二段/);
  await select("值得记住的话");
  await user.keyboard("1");
  expect(await screen.findByRole("status")).toHaveTextContent("标注没有保存：");
  expect(marks()).toEqual([]);
});

// 只看标注 folds unmarked runs into counted lines, opens one run at a time, and `a` or the
// line of facts leads back; with no annotations it is unavailable.
// Core test case:
// - `specs/test-cases/web/stars/annotations.md#focus-mode-must-fold-only-unmarked-blocks`
it("folds everything but the annotations and back", async () => {
  windowWide(true);
  serve([annotation("值得记住的话"), annotation("code", { style: "wavy" })]);
  const { user } = mountApp("/articles/a1");
  await screen.findByText(/第二段/);
  await waitFor(() => expect(marks()).toHaveLength(2));
  await user.keyboard("a");
  const gaps = () =>
    [...document.querySelectorAll(".fold-gap")].map((gap) => gap.textContent);
  await waitFor(() => expect(gaps()).toEqual(["省略 2 段"]));
  expect(screen.getByText("只看 2 处标注")).toBeInTheDocument();
  await user.click(screen.getByText("省略 2 段"));
  await waitFor(() => expect(gaps()).toEqual([]));
  expect(screen.getByText("第三段也没有。")).toBeVisible();
  await user.click(screen.getByRole("button", { name: "显示全文" }));
  await waitFor(() => expect(screen.getByText("2 处标注")).toBeInTheDocument());
  await user.keyboard("a");
  await waitFor(() => expect(gaps()).toEqual(["省略 2 段"]));
  await user.keyboard("{Escape}");
  await waitFor(() => expect(gaps()).toEqual([]));
});

// Without annotations the menu offers 只看标注 disabled and `a` does nothing.
// Core test case:
// - `specs/test-cases/web/stars/annotations.md#focus-mode-must-fold-only-unmarked-blocks`
it("offers no focus mode without annotations", async () => {
  windowWide(true);
  serve();
  const { user } = mountApp("/articles/a1");
  await screen.findByText(/第二段/);
  (await screen.findByRole("button", { name: "管理文章" })).focus();
  await user.keyboard("{Enter}");
  expect(screen.getByRole("menuitem", { name: "只看标注" })).toHaveAttribute(
    "data-disabled",
  );
  await user.keyboard("{Escape}");
  await user.keyboard("a");
  expect(document.querySelector(".fold-gap")).toBeNull();
  expect(screen.queryByText(/处标注/)).toBeNull();
});

// Messages are scopes of their own: a selection across two is no annotation, a selection in
// one is saved against that message, and focus mode folds the messages without any.
// Core test cases:
// - `specs/test-cases/web/stars/annotations.md#marks-must-land-on-the-text-that-was-selected`
// - `specs/test-cases/web/stars/annotations.md#focus-mode-must-fold-only-unmarked-blocks`
it("annotates one message at a time and folds the others", async () => {
  windowWide(true);
  const sent = serve();
  const { user } = mountApp("/conversations/c1?path=p1");
  await screen.findByRole("heading", { name: "一段对话" });
  // Across two messages.
  const first = document.querySelector('[data-annotation-scope="m1"]')!;
  const second = document.querySelector('[data-annotation-scope="m2"]')!;
  const range = document.createRange();
  range.setStart(scopeTexts(first)[0], 1);
  range.setEnd(scopeTexts(second)[0], 1);
  document.getSelection()!.removeAllRanges();
  document.getSelection()!.addRange(range);
  fireEvent.mouseUp(second);
  await act(() => new Promise((resolve) => setTimeout(resolve)));
  expect(screen.queryByRole("toolbar", { name: "标注" })).toBeNull();

  await select("一句", "m2");
  await user.keyboard("u");
  await waitFor(() => expect(marks()).toEqual([["n1", "一句"]]));
  expect(sent[0].body).toEqual({
    message_id: "m2",
    start: 1,
    end: 3,
    quote: "一句",
    style: "underline",
    note: null,
  });
  await user.keyboard("a");
  await waitFor(() =>
    expect(
      [...document.querySelectorAll("ol.messages > li")].map((item) =>
        item.classList.contains("fold-gap")
          ? item.textContent
          : item.querySelector(".body")?.textContent,
      ),
    ).toEqual(["省略 1 条消息", "答一句", "省略 2 条消息"]),
  );
});
