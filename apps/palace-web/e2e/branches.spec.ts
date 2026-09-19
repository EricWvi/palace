import { expect, test } from "@playwright/test";

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
  const messages = [
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
                parent_message_id: chain[index - 1] ?? null,
                created_order: index,
              },
            ] as const,
        ),
      ),
    ).values(),
  ];
  const occurred = new Date(2024, 2, 5, 9, 30).getTime();
  const detail = {
    conversation: { id: "tree", title: "一棵讨论树", source: "chatgpt" },
    messages,
    paths: chains.map((chain, index) => ({
      id: `p${index + 1}`,
      session_id: `s${index + 1}`,
      head_message_id: chain.at(-1),
      message_count: chain.length,
      occurred_at: occurred,
      created_at: 1000,
      updated_at: [1000, 3000, 4000, 2000, 2000][index],
      original_link: `https://chatgpt.com/c/s${index + 1}`,
    })),
  };
  await page.route("**/api/**", async (route) => {
    const path = new URL(route.request().url()).pathname;
    const body =
      path === "/api/conversations"
        ? [
            {
              ...detail.conversation,
              occurred_at: occurred,
              session_ids: detail.paths.map((p) => p.session_id),
              path_count: 5,
              path_id: "p3",
              head_message_id: "A5",
              message_count: messages.length,
            },
          ]
        : detail;
    await route.fulfill({ json: body });
  });
  await page.goto("/conversations/tree");
  await expect(page.getByRole("link", { name: "继续对话" })).toHaveAttribute(
    "href",
    "https://chatgpt.com/c/s3",
  );
  await page
    .getByRole("combobox", { name: "第 4 条消息后的分支" })
    .selectOption("message:U4");
  await expect(page.getByRole("link", { name: "继续对话" })).toHaveAttribute(
    "href",
    "https://chatgpt.com/c/s2",
  );
  await page
    .getByRole("combobox", { name: "第 2 条消息后的分支" })
    .selectOption("message:U2");
  await page
    .getByRole("combobox", { name: "第 2 条消息后的分支" })
    .selectOption("message:U3");
  await expect(
    page.getByRole("combobox", { name: "第 4 条消息后的分支" }),
  ).toHaveValue("message:U5");
  await page
    .getByRole("combobox", { name: "第 2 条消息后的分支" })
    .selectOption("path:p4");
  await page.reload();
  await expect(page.locator(".message")).toHaveCount(2);
  await expect(page.getByRole("link", { name: "继续对话" })).toHaveAttribute(
    "href",
    "https://chatgpt.com/c/s4",
  );
  await page.getByRole("link", { name: "返回会话收藏" }).click();
  await page.getByRole("button", { name: "会话菜单 一棵讨论树" }).click();
  await page.getByRole("menuitem", { name: "分支管理" }).click();
  const manager = page.getByRole("dialog", { name: "分支管理" });
  await expect(manager.locator(".branch-node-text")).toHaveCount(5);
  await expect(
    manager.getByRole("button", { name: "更新分支 s4" }),
  ).toBeVisible();
  await expect(
    manager.getByRole("button", { name: "更新分支 s5" }),
  ).toBeVisible();
  await page.setViewportSize({ width: 390, height: 844 });
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
  await manager.getByRole("button", { name: "更新分支 s2" }).click();
  const update = page.getByRole("dialog", { name: "更新分支" });
  await expect(update.getByLabel("自定义标题")).toBeDisabled();
  await expect(update.getByLabel("会话来源")).toBeDisabled();
  await expect(update.getByLabel("来源网站 Session ID")).toBeDisabled();
  await expect(
    update.getByRole("button", { name: "选择对话发生日期" }),
  ).toHaveText("2024 年 03 月 05 日");
  await update.getByLabel("对话发生时间", { exact: true }).fill("10:45");
  await update.getByLabel("选择会话 JSON 文件").setInputFiles({
    name: "updated.json",
    mimeType: "application/json",
    buffer: Buffer.from('[{"role":"user","content":"U1"}]'),
  });
  await page.route("**/api/conversations/tree/paths/p2", async (route) => {
    await route.fulfill({
      json: { conversation_id: "tree", path_id: "p2", head_message_id: "A4" },
    });
  });
  const upload = page.waitForRequest("**/api/conversations/tree/paths/p2");
  await update.getByRole("button", { name: "保存更新" }).click();
  const request = await upload;
  const expectedDate = new Date(2024, 2, 5, 10, 45).getTime();
  expect(request.postDataJSON()).toEqual({
    history: '[{"role":"user","content":"U1"}]',
    occurred_at: expectedDate,
    idempotency_key: expect.any(String),
  });
  await expect(update).not.toBeVisible();
});
