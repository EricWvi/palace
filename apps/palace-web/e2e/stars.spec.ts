import { expect, test, type Page } from "@playwright/test";
import type { ConversationListItem, Detail } from "../src/lib/api";

// 120 single-path conversations, served 50 to a page the way the server pages them.
function serve(page: Page) {
  let rows: ConversationListItem[] = Array.from({ length: 120 }, (_, i) => ({
    id: `p${i}`,
    conversation_id: `c${i}`,
    title: `对话 ${i}`,
    source: "chatgpt",
    updated_at: new Date(2026, 9, 1, 12).getTime() - i * 3_600_000,
  }));
  const detail = (row: ConversationListItem): Detail => ({
    conversation: {
      id: row.conversation_id,
      owner_id: "owner",
      source: "chatgpt",
    },
    messages: [
      {
        id: `m${row.id}`,
        owner_id: "owner",
        conversation_id: row.conversation_id,
        parent_message_id: null,
        role: "user",
        content: "这个周末想出去走走，最好在海边。",
        toc_line: "这个周末想出去走走",
        created_order: 1,
      },
    ],
    paths: [
      {
        id: row.id,
        title: row.title,
        session_id: row.id,
        head_message_id: `m${row.id}`,
        message_count: 1,
        occurred_at: row.updated_at,
        created_at: row.updated_at,
        updated_at: row.updated_at,
        original_link: `https://chatgpt.com/c/${row.id}`,
      },
    ],
  });
  return page.route("**/api/**", async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    if (url.pathname === "/api/conversations") {
      const start = Number(url.searchParams.get("cursor") ?? 0);
      return route.fulfill({
        json: {
          items: rows.slice(start, start + 50),
          next_cursor: start + 50 < rows.length ? String(start + 50) : null,
          total: rows.length,
        },
      });
    }
    const id = url.pathname.split("/")[3];
    if (request.method() === "DELETE") {
      rows = rows.filter((row) => row.conversation_id !== id);
      return route.fulfill({ json: { id } });
    }
    const row = rows.find((row) => row.conversation_id === id);
    return row
      ? route.fulfill({ json: detail(row) })
      : route.fulfill({ status: 404, json: { error: "not_found" } });
  });
}

// Core test case: `specs/test-cases/web/stars/contents-page.md#returning-to-the-list-must-restore-the-search-and-drop-stale-rows`
test("opens a row at the top and comes back to where the list was left", async ({
  page,
}) => {
  await serve(page);
  await page.goto("/conversations");
  const entries = page.locator(".entry");
  await expect(entries).toHaveCount(50);
  // The second page loads as the end of the first comes into view.
  await entries.nth(49).scrollIntoViewIfNeeded();
  await expect(entries).toHaveCount(100);
  await entries.nth(70).scrollIntoViewIfNeeded();
  await page.waitForTimeout(100);
  const offset = await page.evaluate(() => window.scrollY);
  expect(offset).toBeGreaterThan(1000);

  await entries.nth(70).click();
  await expect(
    page.getByRole("heading", { level: 1, name: "对话 70" }),
  ).toBeVisible();
  expect(await page.evaluate(() => window.scrollY)).toBe(0);

  await page.goBack();
  await expect(page).toHaveURL(/\/conversations$/);
  await expect(entries).toHaveCount(100);
  await expect.poll(() => page.evaluate(() => window.scrollY)).toBe(offset);
});

// Core test cases:
// - `specs/test-cases/web/stars/contents-page.md#returning-to-the-list-must-restore-the-search-and-drop-stale-rows`
// - `specs/test-cases/server/conversation/reading-page.md#deleting-a-conversation-must-return-to-where-the-reader-came-from`
test("deleting a conversation opened from the list returns to a list without it", async ({
  page,
}) => {
  await serve(page);
  await page.goto("/conversations");
  await page.getByRole("link", { name: /^对话 3\b/ }).click();
  await expect(
    page.getByRole("heading", { level: 1, name: "对话 3" }),
  ).toBeVisible();
  await page.getByRole("heading", { level: 1 }).hover();
  await page.getByRole("button", { name: "管理对话" }).click();
  await page.getByRole("menuitem", { name: "删除对话" }).click();
  await page
    .getByRole("dialog", { name: "删除对话？" })
    .getByRole("button", { name: "删除对话" })
    .click();
  await expect(page).toHaveURL(/\/conversations$/);
  await expect(page.locator(".entry").first()).toBeVisible();
  await expect(page.getByRole("link", { name: /^对话 3\b/ })).toHaveCount(0);
  // The deleted page was replaced, so back leaves the list instead of reopening it.
  await page.goBack();
  await expect(page).not.toHaveURL(/\/conversations\/c3/);
});
