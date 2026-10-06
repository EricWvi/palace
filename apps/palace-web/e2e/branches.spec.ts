import { expect, test } from "@playwright/test";
import type { Message, Detail } from "../src/lib/api";

// Core test cases:
// - `specs/test-cases/server/conversation/message-tree.md#shared-and-internal-endpoint-paths-must-remain-independently-manageable`
// - `specs/test-cases/server/conversation/message-tree.md#fork-selection-must-resolve-to-one-real-source-session`
test("nested forks, internal endpoints, branch forms and mixed-language tree labels", async ({
  page,
}) => {
  const chains = [
    ["U1", "A1", "U2", "A2"],
    ["U1", "A1", "U3", "A3", "U4", "A4"],
    ["U1", "A1", "U3", "A3", "U5", "A5"],
    ["U1", "A1"],
    ["U1", "A1"],
  ];
  const messages: Message[] = [
    ...new Map(
      chains.flatMap((chain) =>
        chain.map(
          (id, index) =>
            [
              id,
              {
                id,
                role: id.startsWith("U") ? "user" : "assistant",
                content:
                  id === "U1"
                    ? "中英文 mixed language very long question 对话内容".repeat(
                        8,
                      )
                    : id,
                owner_id: "owner",
                conversation_id: "tree",
                parent_message_id: chain[index - 1] ?? null,
                created_order: index,
              },
            ] as const,
        ),
      ),
    ).values(),
  ];
  const occurred = new Date(2024, 2, 5, 9, 30).getTime();
  const detail: Detail = {
    conversation: {
      owner_id: "owner",
      id: "tree",
      source: "chatgpt",
    },
    messages,
    paths: chains.map((chain, index) => ({
      id: `p${index + 1}`,
      title: `第 ${index + 1} 支`,
      session_id: `s${index + 1}`,
      head_message_id: chain.at(-1)!,
      message_count: chain.length,
      occurred_at: occurred,
      created_at: 1000,
      updated_at: [1000, 3000, 4000, 2000, 2000][index],
      original_link: `https://chatgpt.com/c/s${index + 1}`,
    })),
  };
  await page.route("**/api/**", async (route) => {
    await route.fulfill({ json: detail });
  });
  await page.goto("/conversations/tree");
  const fork = (after: number) =>
    page.getByRole("group", { name: `第 ${after} 条消息后的分支` });
  await expect(page.getByRole("link", { name: "继续对话 ↗" })).toHaveAttribute(
    "href",
    "https://chatgpt.com/c/s3",
  );
  await fork(4).getByRole("button", { name: "U4" }).click();
  await expect(page.getByRole("link", { name: "继续对话 ↗" })).toHaveAttribute(
    "href",
    "https://chatgpt.com/c/s2",
  );
  await expect(page.getByRole("heading", { level: 1 })).toHaveText("第 2 支");
  await fork(2).getByRole("button", { name: "U2" }).click();
  await fork(2).getByRole("button", { name: "U3" }).click();
  await expect(fork(4).getByRole("button", { name: "U5" })).toHaveAttribute(
    "aria-current",
    "true",
  );
  await fork(2).getByRole("button", { name: "在此结束 · s4" }).click();
  await page.reload();
  await expect(page.locator(".message")).toHaveCount(2);
  await expect(page.getByRole("link", { name: "继续对话 ↗" })).toHaveAttribute(
    "href",
    "https://chatgpt.com/c/s4",
  );
  await page.getByRole("heading", { level: 1 }).hover();
  await page.getByRole("button", { name: "管理对话" }).click();
  await page.getByRole("menuitem", { name: "分支管理" }).click();
  const manager = page.getByRole("dialog", { name: "分支管理" });
  await expect(manager.locator(".branch-node-text")).toHaveCount(5);
  await expect(manager.locator(".react-flow__edge")).toHaveCount(4);
  const root = manager.locator('.react-flow__node[data-id="U1"]');
  const child = manager.locator('.react-flow__node[data-id="U3"]');
  await expect(root).toBeVisible();
  expect((await root.boundingBox())!.y).toBeLessThan(
    (await child.boundingBox())!.y,
  );
  const rootActions = root.locator(".branch-node-buttons").first();
  await expect(rootActions).toHaveCSS("opacity", "0");
  await root.hover();
  await expect(rootActions).toHaveCSS("opacity", "1");
  await page.screenshot({ path: "/tmp/palace-branches-desktop.png" });
  await manager.getByRole("button", { name: "适应画布" }).hover();
  await expect(rootActions).toHaveCSS("opacity", "0");
  await manager.getByRole("button", { name: "更新分支 s4" }).focus();
  await expect(rootActions).toHaveCSS("opacity", "1");
  await expect(
    manager.getByRole("button", { name: "更新分支 s4" }),
  ).toBeVisible();
  await expect(
    manager.getByRole("button", { name: "更新分支 s5" }),
  ).toBeVisible();
  await page.setViewportSize({ width: 390, height: 844 });
  await manager.getByRole("button", { name: "适应画布" }).click();
  expect(
    await manager.evaluate((element) => {
      const rect = element.getBoundingClientRect();
      return {
        left: rect.left >= 0,
        right: rect.right <= innerWidth,
        width: rect.width,
        scrollWidth: element.scrollWidth,
        clientWidth: element.clientWidth,
      };
    }),
  ).toEqual({
    left: true,
    right: true,
    width: 358,
    scrollWidth: 356,
    clientWidth: 356,
  });
  const label = manager.locator(".branch-node-text").first();
  expect(
    await label.evaluate((element) => {
      const style = getComputedStyle(element);
      return {
        whiteSpace: style.whiteSpace,
        overflow: style.overflow,
        ellipsis: style.textOverflow,
        clipped: element.scrollWidth > element.clientWidth,
      };
    }),
  ).toEqual({
    whiteSpace: "nowrap",
    overflow: "hidden",
    ellipsis: "ellipsis",
    clipped: true,
  });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
  await page.screenshot({
    path: "/tmp/palace-branches-mobile.png",
    fullPage: true,
  });
  await manager.locator('.react-flow__node[data-id="U4"]').hover();
  await manager.getByRole("button", { name: "更新分支 s2" }).click();
  const update = page.getByRole("dialog", { name: "更新分支" });
  await expect(update.getByLabel("标题")).toHaveValue("第 2 支");
  await expect(update.getByLabel("来源")).toBeDisabled();
  await expect(update.getByLabel("Session ID", { exact: true })).toBeDisabled();
  await expect(
    update.getByRole("button", { name: "选择对话发生日期" }),
  ).toHaveText("2024 年 03 月 05 日");
  await update.getByLabel("对话发生时间", { exact: true }).fill("10:45");
  await update.getByLabel("对话文件", { exact: true }).setInputFiles({
    name: "updated.json",
    mimeType: "application/json",
    buffer: Buffer.from('[{"role":"user","content":"U1"}]'),
  });
  await page.route("**/api/conversations/tree/paths/p2", async (route) => {
    await route.fulfill({
      json: {
        import_id: "i",
        conversation_id: "tree",
        path_id: "p2",
        head_message_id: "A4",
        created: 0,
        reused: 0,
      },
    });
  });
  const upload = page.waitForRequest("**/api/conversations/tree/paths/p2");
  await update.getByRole("button", { name: "保存更新" }).click();
  const request = await upload;
  const expectedDate = new Date(2024, 2, 5, 10, 45).getTime();
  expect(request.postDataJSON()).toEqual({
    title: "第 2 支",
    history: '[{"role":"user","content":"U1"}]',
    occurred_at: expectedDate,
    idempotency_key: expect.any(String),
  });
  await expect(update).not.toBeVisible();
  // The page now shows the updated branch, dated to its new occurrence.
  await expect(page).toHaveURL(/path=p2&date=2024-03-05$/);
});
