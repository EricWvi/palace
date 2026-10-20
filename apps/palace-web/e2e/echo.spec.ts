import { expect, test, type Page } from "@playwright/test";
import type { ListedJotting } from "../src/lib/api";

// Twelve jottings of uneven length, so the masonry columns differ in height; an upload joins
// the top of the wall.
function serve(page: Page) {
  let listed: ListedJotting[] = Array.from({ length: 12 }, (_, i) => ({
    id: `j${i}`,
    created_at: new Date(2026, 8, 20, 12).getTime() - i * 86_400_000,
    title: null,
    tags: ["生活"],
    body: Array.from(
      { length: (i % 4) + 1 },
      () => `第 ${i} 张札记的一段话。`,
    ).join("\n\n"),
    pending: false,
  }));
  return page.route("**/api/**", async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    if (url.pathname === "/api/jottings" && request.method() === "POST") {
      const body = request.postDataJSON() as ListedJotting;
      listed = [{ ...body, title: null, pending: true }, ...listed];
      return route.fulfill({
        status: 201,
        json: { id: body.id, version: "1" },
      });
    }
    if (url.pathname === "/api/jottings")
      return route.fulfill({
        json: { jottings: listed, total: listed.length, next_cursor: null },
      });
    if (url.pathname === "/api/jottings/overview")
      return route.fulfill({
        json: {
          total: listed.length,
          days: listed.length,
          tags: [{ name: "生活", count: listed.length }],
          daily: [],
        },
      });
    if (url.pathname === "/api/jottings/recall")
      return route.fulfill({ json: { date: "2026-10-20", jottings: [] } });
    return route.fulfill({ status: 404, json: { error: "not_found" } });
  });
}

// Each card's column, by the card's id.
async function columns(page: Page) {
  return page
    .locator(".wall .col")
    .evaluateAll((cols) =>
      Object.fromEntries(
        cols.flatMap((col, i) =>
          [...col.querySelectorAll<HTMLElement>("[data-id]")].map((card) => [
            card.dataset.id,
            i,
          ]),
        ),
      ),
    );
}

// Core test case: `specs/test-cases/web/echo/echo-page.md#typing-right-after-n-must-keep-the-caret-in-place`
test("keeps text typed right after n in order while the composer slides open", async ({
  page,
}) => {
  await serve(page);
  await page.goto("/jottings");
  await expect(page.locator(".wall [data-id]")).toHaveCount(12);
  await page.keyboard.press("n");
  // No waiting for the drawer: the bug only showed while it was still growing.
  await page.keyboard.type("今天读完了第三章 and more 中文");
  await expect(page.getByRole("textbox", { name: "新札记" })).toHaveValue(
    "今天读完了第三章 and more 中文",
  );
});

// Core test case: `specs/test-cases/web/echo/echo-page.md#composing-must-extract-inline-tags-and-keep-the-rest-of-the-body-intact`
test("puts a new jotting on top of the first column and moves no other card", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1280, height: 900 });
  await serve(page);
  await page.goto("/jottings");
  await expect(page.locator(".wall [data-id]")).toHaveCount(12);
  const before = await columns(page);
  expect(new Set(Object.values(before)).size).toBe(3);

  await page.keyboard.press("n");
  await page.keyboard.type("新的一张 #想法");
  await page.keyboard.press("Control+Enter");
  await expect(page.locator(".wall [data-id]")).toHaveCount(13);
  const first = page.locator(".wall .col").first().locator("[data-id]").first();
  await expect(first).toContainText("新的一张");
  await expect(first.getByLabel("待整理")).toBeVisible();
  const after = await columns(page);
  for (const [id, column] of Object.entries(before))
    expect([id, after[id]]).toEqual([id, column]);
});
