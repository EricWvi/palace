import { expect, test, type Page } from "@playwright/test";

const article = {
  id: "a1",
  title: "读书的第二遍",
  content: Array.from({ length: 30 }, () =>
    "读的时候最容易忽略的，是作者没有写出来的部分。".repeat(6),
  ).join("\n\n"),
  url: "https://example.com/post",
  author: "某人",
  site: "example.com",
  published_at: new Date(2026, 9, 5, 12).getTime(),
  date: "2026-10-05",
  occurred_at: new Date(2026, 9, 5, 12).getTime(),
  media: [],
};

// Serves one article with one 总批 and no annotations.
async function serve(page: Page, text: string | null) {
  await page.route("**/api/**", async (route) => {
    const { pathname } = new URL(route.request().url());
    if (pathname === "/api/articles/a1")
      return route.fulfill({ json: article });
    if (pathname === "/api/articles/a1/overall-note")
      return route.fulfill({ json: { text, version: text ? 1 : 0 } });
    if (pathname === "/api/me")
      return route.fulfill({ json: { id: "o", email: "o@example.com" } });
    return route.fulfill({ json: { items: [] } });
  });
}

// jsdom lays nothing out, so where the card sits is checked here.
// Core test case:
// - `specs/test-cases/web/stars/overall-notes.md#the-overall-note-card-must-stay-out-of-the-reading-flow`
test("the card stands in the margin and leaves the text where it was", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await serve(page, null);
  await page.goto("/articles/a1");
  const title = page.getByRole("heading", { level: 1 });
  await expect(title).toBeVisible();
  await expect(page.getByRole("complementary", { name: "总批" })).toHaveCount(
    0,
  );

  await page.unroute("**/api/**");
  await serve(page, "这篇的核心是第二遍才算读。\n\n可以写进札记。");
  await page.reload();
  const card = page.getByRole("complementary", { name: "总批" });
  await expect(card).toContainText("这篇的核心是第二遍才算读。");
  const box = (await card.boundingBox())!;
  const text = (await page.locator(".note-body").boundingBox())!;
  // Past the text column, and pinned to the foot of the window, however far the page scrolls.
  expect(box.x).toBeGreaterThan(text.x + text.width);
  expect(box.y + box.height).toBeCloseTo(900 - 28, 0);
  await page.evaluate(() => scrollTo(0, 1000));
  expect((await card.boundingBox())!.y).toBeCloseTo(box.y, 0);
});

// Core test case:
// - `specs/test-cases/web/stars/overall-notes.md#the-overall-note-card-must-stay-out-of-the-reading-flow`
test("narrower than the margin it folds to its name", async ({ page }) => {
  await page.setViewportSize({ width: 900, height: 800 });
  await serve(page, "这一行不该压住正文");
  await page.goto("/articles/a1");
  const card = page.getByRole("complementary", { name: "总批" });
  await expect(card).toBeVisible();
  await expect(card.locator(".overall-note-first")).toBeHidden();
  expect((await card.boundingBox())!.width).toBeLessThan(100);
});
