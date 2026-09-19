import { test, expect } from "@playwright/test";

// Core test case: `specs/test-cases/server/conversation/message-tree.md#card-menu-metadata-editing-must-refresh-every-visible-projection`
test("desktop and mobile library, calendar import, and routed chat", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  const item = {
    id: "one",
    title: "把零散的灵感，整理成自己的知识体系",
    source: "chatgpt",
    session_ids: ["knowledge-notes"],
    path_id: "path-one",
    path_count: 1,
    occurred_at: 1789648800000,
    head_message_id: "answer",
    message_count: 2,
  };
  let current = { ...item };
  const messages = [
    {
      id: "question",
      parent_message_id: null,
      role: "user",
      content: "如何建立一个可以持续积累的知识库？",
      created_order: 1,
    },
    {
      id: "answer",
      parent_message_id: "question",
      role: "assistant",
      content:
        "## 从一个小问题开始\n\n让知识围绕你关心的问题生长，而不是从分类开始。\n\n1. 收集值得回看的内容\n2. 用自己的话写下理解\n3. 定期建立联系\n\n> 留存只是起点，重新使用才是价值。",
      created_order: 2,
    },
  ];
  await page.route("**/api/**", async (route) => {
    const path = new URL(route.request().url()).pathname;
    if (
      path === "/api/conversations/one" &&
      route.request().method() === "PUT"
    ) {
      current = { ...current, ...route.request().postDataJSON() };
      await route.fulfill({ json: current });
      return;
    }
    const data =
      path === "/api/conversations"
        ? [current]
        : path === "/api/import/file"
          ? {
              conversation_id: "one",
              path_id: "path-one",
              head_message_id: "answer",
            }
          : path.includes("/paths/")
            ? messages
            : {
                conversation: current,
                messages,
                paths: [
                  {
                    id: "path-one",
                    session_id: "knowledge-notes",
                    head_message_id: "answer",
                    occurred_at: item.occurred_at,
                    created_at: item.occurred_at,
                    updated_at: item.occurred_at,
                    message_count: 2,
                    original_link:
                      current.source === "gemini"
                        ? "https://gemini.google.com/app/knowledge-notes"
                        : "https://chatgpt.com/c/knowledge-notes",
                  },
                ],
              };
    await route.fulfill({ json: data });
  });
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.goto("/");
  await expect(page.getByText(item.title)).toBeVisible();
  await page.getByRole("button", { name: `会话菜单 ${item.title}` }).click();
  await page.getByRole("menuitem", { name: "编辑会话" }).click();
  const editor = page.getByRole("dialog", { name: "编辑会话" });
  await editor.getByLabel("会话标题").fill("整理后的知识体系");
  await editor.getByLabel("消息来源").selectOption("gemini");
  const updated = page.waitForRequest(
    (request) =>
      request.method() === "PUT" &&
      new URL(request.url()).pathname === "/api/conversations/one",
  );
  await editor.getByRole("button", { name: "保存更改" }).click();
  expect((await updated).postDataJSON()).toEqual({
    title: "整理后的知识体系",
    source: "gemini",
  });
  await expect(page.getByText("整理后的知识体系")).toBeVisible();
  await page.getByRole("button", { name: "会话菜单 整理后的知识体系" }).click();
  await page.getByRole("menuitem", { name: "分支管理" }).click();
  const manager = page.getByRole("dialog", { name: "分支管理" });
  await expect(manager.getByText(messages[0].content)).toBeVisible();
  await expect(
    manager.getByRole("button", { name: "删除分支 knowledge-notes" }),
  ).toBeDisabled();
  await manager.getByRole("button", { name: "Close", exact: true }).click();
  await page.screenshot({ path: "/tmp/palace-library.png", fullPage: true });
  await page.getByRole("button", { name: "导入会话", exact: true }).click();
  await page.getByLabel("来源网站 Session ID").fill("knowledge-notes");
  await page.getByLabel("自定义标题").fill("思考的下一步");
  await page.getByRole("button", { name: "选择对话发生日期" }).click();
  await expect(page.getByRole("grid")).toBeVisible();
  await page.screenshot({ path: "/tmp/palace-calendar.png" });
  const chosen = new Date();
  chosen.setDate(15);
  const day = await page.evaluate(() => {
    const date = new Date();
    date.setDate(15);
    return date.toLocaleDateString();
  });
  await page.locator(`[data-day="${day}"]`).click();
  await expect(page.getByRole("grid")).not.toBeVisible();
  await page.getByLabel("对话发生时间", { exact: true }).fill("09:30");
  await page.getByLabel("选择会话 JSON 文件").setInputFiles({
    name: "conversation.json",
    mimeType: "application/json",
    buffer: Buffer.from(JSON.stringify([{ role: "user", content: "你好" }])),
  });
  const uploaded = page.waitForRequest("**/api/import/file");
  await page.getByRole("button", { name: "导入并查看会话" }).click();
  const body = (await uploaded).postData()!;
  expect(body).toContain("knowledge-notes");
  chosen.setHours(9, 30, 0, 0);
  expect(body).toContain(String(chosen.getTime()));
  await expect(
    page.getByRole("heading", { name: "从一个小问题开始" }),
  ).toBeVisible();
  await page.screenshot({ path: "/tmp/palace-chat.png", fullPage: true });
  await page.reload();
  await expect(page.getByRole("link", { name: "继续对话" })).toHaveAttribute(
    "href",
    "https://gemini.google.com/app/knowledge-notes",
  );
  await expect(
    page.getByRole("heading", { name: "整理后的知识体系" }),
  ).toBeVisible();
  await page.setViewportSize({ width: 390, height: 844 });
  await page.getByRole("link", { name: "返回会话收藏" }).click();
  await page.getByRole("button", { name: "会话菜单 整理后的知识体系" }).click();
  await page.getByRole("menuitem", { name: "编辑会话" }).click();
  const mobileEditor = page.getByRole("dialog", { name: "编辑会话" });
  await mobileEditor.getByLabel("会话标题").fill("移动端修正标题");
  await mobileEditor.getByLabel("消息来源").selectOption("grok");
  await mobileEditor.getByRole("button", { name: "保存更改" }).click();
  await expect(page.getByText("移动端修正标题")).toBeVisible();
  await page.screenshot({ path: "/tmp/palace-mobile.png", fullPage: true });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
  expect(errors).toEqual([]);
});
