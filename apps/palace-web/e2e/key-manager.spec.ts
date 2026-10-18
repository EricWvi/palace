import { expect, test } from "@playwright/test";
import type { AccessToken } from "../src/lib/api";

const token = `palace_pat_Ab3d${"k".repeat(35)}wXyZ`;
const masked = `palace_pat_Ab3d${"*".repeat(35)}wXyZ`;

// Core test case: `specs/test-cases/server/owner/access-tokens.md#the-key-manager-must-never-show-a-full-token`
test("creates, copies and deletes a key from the day menu without showing it", async ({
  page,
  context,
}) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  let tokens: AccessToken[] = [];
  await page.route("**/api/timeline**", (route) => route.fulfill({ json: [] }));
  await page.route("**/api/access-tokens**", async (route) => {
    const request = route.request();
    if (request.method() === "POST") {
      const created: AccessToken = {
        id: "fresh",
        note: (request.postDataJSON() as { note: string }).note.trim(),
        masked,
        created_at: new Date(2025, 8, 30, 9).getTime(),
        last_used_at: null,
      };
      tokens = [created, ...tokens];
      return route.fulfill({
        status: 201,
        json: { access_token: created, token },
      });
    }
    if (request.method() === "DELETE") {
      expect(request.headers()["origin"]).toBe(new URL(page.url()).origin);
      tokens = tokens.filter(({ id }) => !request.url().endsWith(`/${id}`));
      return route.fulfill({ status: 204 });
    }
    return route.fulfill({ json: tokens });
  });
  await page.goto("/?date=2025-09-30");

  await page.locator("header.day h1").hover();
  await page.getByRole("button", { name: "当天操作" }).click();
  await expect(page.getByRole("menuitem")).toHaveText(["导入对话", "密钥管理"]);
  await page.getByRole("menuitem", { name: "密钥管理" }).click();
  const dialog = page.getByRole("dialog", { name: "密钥管理" });
  await expect(dialog.getByText("还没有密钥。")).toBeVisible();
  await dialog.getByLabel("备注").fill("命令行");
  await dialog.getByRole("button", { name: "生成" }).click();
  const row = dialog.getByRole("listitem").filter({ hasText: "命令行" });
  await expect(row).toContainText(masked);
  await expect(row).toContainText("仅此一次可复制，关闭后无法再次查看");
  await row.getByRole("button", { name: "复制密钥" }).click();
  await expect(row.getByRole("button", { name: "复制密钥" })).toHaveText(
    "已复制",
  );
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(token);
  await expect(page.locator("body")).not.toContainText(token);

  // Reopened, the key can no longer be copied.
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  await page.locator("header.day h1").hover();
  await page.getByRole("button", { name: "当天操作" }).click();
  await page.getByRole("menuitem", { name: "密钥管理" }).click();
  await expect(row).toContainText("创建于 2025-09-30 · 从未使用");
  await expect(row.getByRole("button", { name: "复制密钥" })).toHaveCount(0);

  await row.getByRole("button", { name: "删除密钥 命令行" }).click();
  const confirm = page.getByRole("dialog", { name: "删除密钥？" });
  await confirm.getByRole("button", { name: "确认删除密钥" }).click();
  await expect(confirm).toBeHidden();
  await expect(dialog.getByText("还没有密钥。")).toBeVisible();
});
