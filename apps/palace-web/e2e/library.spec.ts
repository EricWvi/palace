import { test, expect } from "@playwright/test";

test("desktop and mobile library, calendar import, and routed chat", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  const item = {
    id: "one",
    title: "把零散的灵感，整理成自己的知识体系",
    source: "chatgpt",
    session_id: "knowledge-notes",
    imported_at: 1789648800000,
    head_message_id: "answer",
    message_count: 2,
  };
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
    const data =
      path === "/api/conversations"
        ? [item]
        : path === "/api/import/file"
          ? { conversation_id: "one", head_message_id: "answer" }
          : path.includes("/paths/")
            ? messages
            : {
                conversation: item,
                messages,
                original_link: "https://chatgpt.com/c/knowledge-notes",
              };
    await route.fulfill({ json: data });
  });
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.goto("/");
  await expect(page.getByText(item.title)).toBeVisible();
  await page.screenshot({ path: "/tmp/palace-library.png", fullPage: true });
  await page.getByRole("button", { name: "导入会话", exact: true }).click();
  await page.getByLabel("来源网站 Session ID").fill("knowledge-notes");
  await page.getByLabel("自定义标题").fill("思考的下一步");
  await page.getByRole("button", { name: "选择导入日期" }).click();
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
  await page.keyboard.press("Escape");
  await page.getByLabel("导入时间", { exact: true }).fill("09:30");
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
  await expect(page.getByRole("heading", { name: item.title })).toBeVisible();
  await page.setViewportSize({ width: 390, height: 844 });
  await page.screenshot({ path: "/tmp/palace-mobile.png", fullPage: true });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
  expect(errors).toEqual([]);
});
