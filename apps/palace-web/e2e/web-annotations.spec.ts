import { expect, test, type Page } from "@playwright/test";
import type { Annotation, Article, WebAnnotation } from "../src/lib/api";

const paragraphs = [
  "收藏夹里躺着的文章，大多数再也没有被打开过。存下来只是延后了判断，真正的阅读需要在文字上留下一点痕迹。",
  "读完之后写一段摘要，看起来是在整理，其实常常是在复述作者的结构。划线和旁批不一样：它们发生在阅读的当下。",
  "这也是为什么纸书的页边那么重要。页边是作者留给读者的空间，窄，但足够写下一句反驳、一个问号。",
];
const article: Article = {
  id: "a1",
  title: "把一篇长文读慢一点",
  content: paragraphs.join("\n\n"),
  url: "https://example.com/slow",
  author: "林间",
  site: "example.com",
  published_at: new Date(2024, 4, 3).getTime(),
  date: "2024-05-03",
  occurred_at: new Date(2026, 9, 20, 12).getTime(),
  media: [],
};
// The annotation text of the body: the paragraphs with the newline between blocks.
const text = paragraphs.join("\n");

// What the extension saved on the original page, whose DOM text indents every paragraph.
function web(
  id: string,
  quote: string,
  prefix: string,
  suffix: string,
  fields: Partial<WebAnnotation>,
): WebAnnotation {
  return {
    id,
    page_id: "p1",
    quote,
    prefix,
    suffix,
    position: 0.5,
    style: null,
    note: null,
    created_at: 7,
    updated_at: 7,
    ...fields,
  };
}

// Serves the article and keeps both kinds of annotation in memory, migrating as the server
// would: each listed web annotation that still exists becomes one article annotation.
async function serve(page: Page, initial: WebAnnotation[]) {
  let pending = [...initial];
  let stored: Annotation[] = [];
  const migrations: unknown[] = [];
  await page.route("**/api/**", async (route) => {
    const request = route.request();
    const path = new URL(request.url()).pathname;
    if (path === "/api/articles/a1") return route.fulfill({ json: article });
    if (path === "/api/articles/a1/annotations")
      return route.fulfill({ json: { items: stored } });
    if (path === "/api/articles/a1/web-annotations")
      return route.fulfill({ json: { items: pending } });
    if (path === "/api/articles/a1/web-annotations/migrate") {
      const { items } = request.postDataJSON() as {
        items: {
          web_annotation_id: string;
          start: number;
          end: number;
          quote: string;
        }[];
      };
      migrations.push(items);
      const created = items.flatMap((item) => {
        const source = pending.find(
          (candidate) => candidate.id === item.web_annotation_id,
        );
        if (!source) return [];
        return [
          {
            id: `n-${source.id}`,
            message_id: null,
            start: item.start,
            end: item.end,
            quote: item.quote,
            style: source.style,
            note: source.note,
            created_at: source.created_at,
            updated_at: 9,
          } satisfies Annotation,
        ];
      });
      const moved = new Set(items.map((item) => item.web_annotation_id));
      pending = pending.filter((item) => !moved.has(item.id));
      stored = [...stored, ...created];
      return route.fulfill({ json: { items: created } });
    }
    if (
      path.startsWith("/api/web-annotations/") &&
      request.method() === "DELETE"
    ) {
      const id = path.split("/").at(-1);
      pending = pending.filter((item) => item.id !== id);
      return route.fulfill({ status: 204 });
    }
    return route.fulfill({ json: [] });
  });
  return { migrations, pending: () => pending };
}

// The text each mark of the page draws, joined per annotation.
const marked = (page: Page) =>
  page.locator("mark.anno").evaluateAll((marks) => {
    const texts: Record<string, string> = {};
    for (const mark of marks as HTMLElement[]) {
      const id = mark.dataset.annotation!;
      texts[id] = (texts[id] ?? "") + mark.textContent;
    }
    return texts;
  });

// Core test case:
// - `specs/test-cases/extension/web-annotations.md#relocation-must-anchor-a-web-annotation-on-the-same-words-in-the-rendered-article`
test("web annotations migrate onto the same words, and the unmatched one can be discarded", async ({
  page,
}) => {
  const server = await serve(page, [
    // Across two paragraphs, with the page's newline and indentation between them.
    web(
      "w1",
      "阅读的当下。\n\n      这也是为什么",
      "旁批不一样：它们发生在",
      "纸书的页边那么重要",
      { style: "rose", note: "这一段是核心。" },
    ),
    // "阅读" appears twice; only the first agrees with its context.
    web("w2", "阅读", "，真正的", "需要在文字上", { style: "underline" }),
    // The importer changed these words, so there is nothing to match.
    web("w3", "页边是读者留给作者的空间", "", "", { style: "yellow" }),
  ]);
  await page.goto("/articles/a1");

  await expect(page.getByText("2 处标注")).toBeVisible();
  await expect
    .poll(() => marked(page))
    .toEqual({
      "n-w1": "阅读的当下。\n这也是为什么",
      "n-w2": "阅读",
    });
  // Every anchor the page sent is the body text at its offsets.
  const [sent] = server.migrations as {
    start: number;
    end: number;
    quote: string;
  }[][];
  expect(
    sent.map(({ start, end, quote }) => [text.slice(start, end), quote]),
  ).toEqual([
    ["阅读的当下。\n这也是为什么", "阅读的当下。\n这也是为什么"],
    ["阅读", "阅读"],
  ]);
  expect(sent[1].start).toBe(text.indexOf("阅读"));

  const unmatched = page.getByRole("button", {
    name: "1 处原网页标注没对上",
  });
  await unmatched.click();
  const list = page.locator(".unmatched");
  await expect(list.locator("blockquote")).toHaveText([
    "页边是读者留给作者的空间",
  ]);
  await page.screenshot({ path: "test-results/web-annotations-unmatched.png" });
  await list.getByRole("button", { name: "丢弃" }).click();
  await expect(unmatched).toHaveCount(0);
  expect(server.pending()).toEqual([]);
  // The body never changes, so nothing is looked for again.
  expect(server.migrations).toHaveLength(1);
});
