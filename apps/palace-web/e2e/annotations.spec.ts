import { expect, test, type Page } from "@playwright/test";
import type { Annotation, Article } from "../src/lib/api";

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

function annotation(
  id: string,
  quote: string,
  fields: Partial<Annotation>,
): Annotation {
  const start = text.indexOf(quote);
  return {
    id,
    message_id: null,
    start,
    end: start + quote.length,
    quote,
    style: null,
    note: null,
    created_at: 1,
    updated_at: 1,
    ...fields,
  };
}

// Serves the article and keeps its annotations in memory, as the server would.
async function serve(page: Page, initial: Annotation[], served = article) {
  let stored = [...initial];
  let next = 1;
  await page.route("**/api/**", async (route) => {
    const request = route.request();
    const path = new URL(request.url()).pathname;
    if (path === "/api/articles/a1") return route.fulfill({ json: served });
    if (path === "/api/articles/a1/annotations" && request.method() === "GET")
      return route.fulfill({ json: { items: stored } });
    if (path === "/api/articles/a1/annotations") {
      const created = {
        ...annotation(`n${next++}`, "", {}),
        ...request.postDataJSON(),
      };
      stored = [...stored, created];
      return route.fulfill({ status: 201, json: created });
    }
    const id = path.split("/").at(-1)!;
    if (request.method() === "PUT") {
      stored = stored.map((item) =>
        item.id === id ? { ...item, ...request.postDataJSON() } : item,
      );
      return route.fulfill({ json: stored.find((item) => item.id === id) });
    }
    if (request.method() === "DELETE") {
      stored = stored.filter((item) => item.id !== id);
      return route.fulfill({ status: 204 });
    }
    return route.fulfill({ json: [] });
  });
}

// Selects a quote in the body the way a drag would end, then lets go of the mouse.
async function select(page: Page, quote: string) {
  await page.evaluate((quote) => {
    const root = document.querySelector("[data-annotation-scope]")!;
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      const at = (node as Text).data.indexOf(quote);
      if (at === -1) continue;
      const range = document.createRange();
      range.setStart(node, at);
      range.setEnd(node, at + quote.length);
      getSelection()!.removeAllRanges();
      getSelection()!.addRange(range);
      root.dispatchEvent(new MouseEvent("mouseup", { bubbles: true }));
      return;
    }
  }, quote);
  await expect(page.getByRole("toolbar", { name: "标注" })).toBeVisible();
}

const sheetWidth = (page: Page) =>
  page
    .locator("main.sheet")
    .evaluate((element) => Math.round(element.getBoundingClientRect().width));
const box = async (page: Page, selector: string) =>
  (await page.locator(selector).first().boundingBox())!;

// Core test case:
// - `specs/test-cases/web/stars/annotations.md#margin-notes-must-widen-the-page-only-while-notes-exist`
test("the margin opens with the first note, keeps cards level with their marks and closes with the last", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  await serve(page, [
    annotation("h1", "存下来只是延后了判断", { style: "yellow" }),
  ]);
  await page.goto("/articles/a1");
  await expect(page.locator("mark.anno")).toHaveText(["存下来只是延后了判断"]);
  // A highlight alone keeps the page as wide as every other reading page.
  await expect.poll(() => sheetWidth(page)).toBe(760);
  const column = await box(page, ".annotated");

  await select(page, "划线和旁批不一样");
  await page.keyboard.press("c");
  await page.keyboard.type("这句可以放进摘星首页当引语。");
  await page.keyboard.press("Control+Enter");
  await expect(page.locator(".margin-notes .note-card")).toHaveText([
    "这句可以放进摘星首页当引语。",
  ]);
  // The sheet widens the way 回响 does; the text column keeps its width and left edge.
  expect(
    await page
      .locator("main.sheet")
      .evaluate((element) => getComputedStyle(element).transitionDuration),
  ).toBe("0.4s");
  await expect.poll(() => sheetWidth(page)).toBe(1064);
  const widened = await box(page, ".annotated");
  expect(Math.round(widened.width)).toBe(Math.round(column.width));

  // The card sits in the margin, level with its mark's first line.
  const mark = await box(page, 'mark.anno[data-annotation="n1"]');
  const card = await box(page, ".margin-notes .note-card");
  expect(Math.round(card.x - (widened.x + widened.width))).toBe(56);
  expect(Math.abs(card.y - (mark.y - 9))).toBeLessThanOrEqual(2);

  // A second note on the same line is pushed below the first instead of covering it.
  await select(page, "看起来是在整理");
  await page.keyboard.press("c");
  await page.keyboard.type("同意。");
  await page.keyboard.press("Control+Enter");
  const cards = page.locator(".margin-notes .note-card");
  await expect(cards).toHaveCount(2);
  await expect
    .poll(async () => {
      const [first, second] = await cards.evaluateAll((elements) =>
        elements
          .map((element) => element.getBoundingClientRect())
          .sort((a, b) => a.top - b.top)
          .map((rect) => ({ top: rect.top, bottom: rect.bottom })),
      );
      return Math.round(second.top - first.bottom);
    })
    .toBeGreaterThanOrEqual(10);

  // Hovering the words lifts their card toward them, and the other way round.
  await page.locator('mark.anno[data-annotation="n1"]').hover();
  const first = page.locator(".margin-notes .note-card", {
    hasText: "这句可以放进",
  });
  await expect(first).toHaveClass(/hot/);
  await expect
    .poll(() =>
      first.evaluate((element) => getComputedStyle(element).transform),
    )
    .toBe("matrix(1, 0, 0, 1, -4, 0)");
  await page.screenshot({ path: "test-results/annotations-margin.png" });
  await page.mouse.move(5, 5);
  await expect(first).not.toHaveClass(/hot/);
  await first.hover();
  await expect(
    page.locator('mark.anno[data-annotation="n1"]').first(),
  ).toHaveClass(/hot/);

  // Emptying both notes closes the margin and narrows the page back.
  for (const note of ["这句可以放进", "同意。"]) {
    await page.locator(".margin-notes .note-card", { hasText: note }).click();
    await page.getByRole("textbox", { name: "批注" }).fill("");
    await page.keyboard.press("Escape");
  }
  await expect(page.locator(".margin-notes")).toHaveCount(0);
  await expect.poll(() => sheetWidth(page)).toBe(760);
});

// Core test case:
// - `specs/test-cases/web/stars/annotations.md#margin-notes-must-widen-the-page-only-while-notes-exist`
test("a narrow window floats a note under its mark instead of widening", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1000, height: 900 });
  await serve(page, [
    annotation("n1", "页边是作者留给读者的空间", {
      note: "所以批注放在右边。",
    }),
  ]);
  await page.goto("/articles/a1");
  const mark = page.locator("mark.anno");
  await expect(mark).toHaveText(["页边是作者留给读者的空间"]);
  await expect(page.locator(".margin-notes")).toHaveCount(0);
  await expect.poll(() => sheetWidth(page)).toBe(760);
  await mark.hover();
  const card = page.locator(".note-card.floating");
  await expect(card).toHaveText("所以批注放在右边。");
  const under = (await mark.boundingBox())!;
  const floating = (await card.boundingBox())!;
  expect(Math.round(floating.y - (under.y + under.height))).toBe(8);
  await page.screenshot({ path: "test-results/annotations-floating.png" });
});

// The first note on a page, or the first added to a highlight, opens the margin where the reader
// is reading: the page never jumps to the top to show a card not yet placed.
// Core test case:
// - `specs/test-cases/web/stars/annotations.md#margin-notes-must-widen-the-page-only-while-notes-exist`
test("the first note opens beside its words without moving the page", async ({
  page,
}) => {
  // Long enough that the words read are far below the top of the margin column.
  const padding = Array<string>(8).fill("铺垫的一段话，让页面足够长。");
  const long = {
    ...article,
    content: [...padding, ...paragraphs].join("\n\n"),
  };
  const quote = "页边是作者留给读者的空间";
  const start = [...padding, ...paragraphs].join("\n").indexOf(quote);
  await page.setViewportSize({ width: 1400, height: 360 });
  await serve(
    page,
    [
      annotation("h1", quote, {
        style: "yellow",
        start,
        end: start + quote.length,
      }),
    ],
    long,
  );
  await page.goto("/articles/a1");
  await expect(page.locator("mark.anno")).toHaveText([quote]);
  await page.evaluate(() => scrollTo(0, document.body.scrollHeight));
  const scrolled = await page.evaluate(() => scrollY);
  expect(scrolled).toBeGreaterThan(0);

  await page.locator("mark.anno").click();
  await page.keyboard.press("c");
  const field = page.getByRole("textbox", { name: "批注" });
  await expect(field).toBeFocused();
  await expect(field).toBeInViewport();
  expect(await page.evaluate(() => scrollY)).toBe(scrolled);
  await page.keyboard.press("Escape");
  await expect(page.locator(".margin-notes")).toHaveCount(0);

  await select(page, "窄，但足够写下一句反驳");
  await page.keyboard.press("c");
  await expect(field).toBeFocused();
  await expect(field).toBeInViewport();
  expect(await page.evaluate(() => scrollY)).toBe(scrolled);
});
