import { expect, test, type Page } from "@playwright/test";
import type { Detail, Moment } from "../src/lib/api";
import type { components } from "../src/lib/generated/api";

type Path = Detail["paths"][number];
// A small in-memory Palace: conversations with titled paths, and moments derived from them.
function serve(page: Page) {
  const at = (month: number, date: number, hour: number) =>
    new Date(2025, month - 1, date, hour).getTime();
  const conversations = new Map<string, Detail>([
    [
      "c1",
      {
        conversation: { id: "c1", owner_id: "owner", source: "chatgpt" },
        messages: [
          {
            id: "q",
            owner_id: "owner",
            conversation_id: "c1",
            parent_message_id: null,
            role: "user",
            content: "这个周末想出去走走，最好在海边。",
            created_order: 1,
          },
          {
            id: "a",
            owner_id: "owner",
            conversation_id: "c1",
            parent_message_id: "q",
            role: "assistant",
            content: "## 三个选择\n\n- **Half Moon Bay**\n- **Point Reyes**",
            created_order: 2,
          },
        ],
        paths: [
          {
            id: "p1",
            title: "计划周末出行",
            session_id: "trip",
            head_message_id: "a",
            message_count: 2,
            occurred_at: at(9, 30, 10),
            created_at: 1,
            updated_at: 1,
            original_link: "https://chatgpt.com/c/trip",
          },
        ],
      },
    ],
  ]);
  const moments = (): Moment[] =>
    [...conversations.values()].flatMap(({ conversation, messages, paths }) =>
      paths.map((path: Path) => ({
        kind: "conversation" as const,
        id: path.id,
        occurred_at: path.occurred_at,
        conversation_id: conversation.id,
        title: path.title,
        source: conversation.source,
        message_count: path.message_count,
        excerpt: messages.slice(0, 2).map((message) => ({
          role: message.role,
          text: message.content.replace(/[#*-]/g, "").trim(),
        })),
      })),
    );
  const requests: { method: string; path: string; body: unknown }[] = [];
  return page
    .route("**/api/**", async (route) => {
      const request = route.request();
      const url = new URL(request.url());
      const path = url.pathname;
      if (path === "/api/timeline") {
        const start = Number(url.searchParams.get("start"));
        const end = Number(url.searchParams.get("end"));
        return route.fulfill({
          json: moments().filter(
            (moment) => moment.occurred_at >= start && moment.occurred_at < end,
          ),
        });
      }
      if (path === "/api/import/file") {
        const form = await new Response(
          new Uint8Array(request.postDataBuffer()!),
          { headers: { "Content-Type": request.headers()["content-type"] } },
        ).formData();
        requests.push({ method: "POST", path, body: Object.fromEntries(form) });
        conversations.set("c2", {
          conversation: {
            id: "c2",
            owner_id: "owner",
            source: form.get("source") as Detail["conversation"]["source"],
          },
          messages: [
            {
              id: "n",
              owner_id: "owner",
              conversation_id: "c2",
              parent_message_id: null,
              role: "user",
              content: "你好",
              created_order: 1,
            },
          ],
          paths: [
            {
              id: "p2",
              title: String(form.get("title")),
              session_id: String(form.get("session_id")),
              head_message_id: "n",
              message_count: 1,
              occurred_at: Number(form.get("occurred_at")),
              created_at: 1,
              updated_at: 1,
              original_link: `https://chatgpt.com/c/${form.get("session_id")}`,
            },
          ],
        });
        return route.fulfill({
          json: {
            import_id: "i",
            conversation_id: "c2",
            path_id: "p2",
            head_message_id: "n",
            created: 1,
            reused: 0,
          },
        });
      }
      const [, , , id, , pathId, field] = path.split("/");
      const detail = conversations.get(id);
      if (!detail) return route.fulfill({ status: 404, json: {} });
      if (field === "metadata") {
        const body: components["schemas"]["PathMetadata"] =
          request.postDataJSON();
        requests.push({ method: "PUT", path, body });
        detail.conversation.source = body.source;
        detail.paths = detail.paths.map((p) =>
          p.id === pathId ? { ...p, title: body.title } : p,
        );
        return route.fulfill({
          json: { conversation_id: id, path_id: pathId, ...body },
        });
      }
      if (request.method() === "DELETE") {
        requests.push({ method: "DELETE", path, body: null });
        conversations.delete(id);
        return route.fulfill({ json: { id } });
      }
      return route.fulfill({ json: detail });
    })
    .then(() => requests);
}

// Core test cases:
// - `specs/test-cases/web/navigation/text-nav-and-day-routes.md#returning-from-a-conversation-must-land-on-the-same-day-without-focusing-the-moment`
// - `specs/test-cases/web/navigation/text-nav-and-day-routes.md#hover-menus-must-stay-reachable-by-keyboard-and-touch`
// - `specs/test-cases/server/import/import-entry.md#successful-import-must-open-the-reading-page-dated-to-the-submitted-occurrence`
// - `specs/test-cases/server/conversation/reading-page.md#editing-must-update-the-current-path-title-and-the-conversation-source-everywhere`
// - `specs/test-cases/server/conversation/reading-page.md#deleting-a-conversation-must-return-to-where-the-reader-came-from`
test("a day, its conversation, an import filed on another day, and the way back", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  const requests = await serve(page);
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto("/?date=2025-09-30");
  const card = page.getByRole("link", { name: /计划周末出行/ });
  await expect(card).toBeVisible();
  // Only 时刻 is a link; the other sections are words.
  await expect(
    page.getByRole("navigation", { name: "主导航" }).getByRole("link"),
  ).toHaveText(["时刻"]);

  // The day menu shows only while the pointer is over the date.
  const dayMenu = page.getByRole("button", { name: "当天操作" });
  await expect(dayMenu).toHaveCSS("opacity", "0");
  await page.locator("header.day h1").hover();
  await expect(dayMenu).toHaveCSS("opacity", "1");

  // The conversation title sits exactly where the day heading was.
  const dayTop = (await page.locator("header.day h1").boundingBox())!.y;
  await card.click();
  await expect(page).toHaveURL(/\/conversations\/c1\?path=p1&date=2025-09-30$/);
  const title = page.getByRole("heading", { level: 1, name: "计划周末出行" });
  await expect(title).toBeVisible();
  expect((await title.boundingBox())!.y).toBe(dayTop);
  await expect(page).toHaveTitle("计划周末出行");
  await page.screenshot({ path: "/tmp/palace-reading.png", fullPage: true });

  // Back lands on the day without lighting up the card it came from.
  await page.getByRole("link", { name: "← 9 月 30 日" }).click();
  await expect(page).toHaveURL(/\/\?date=2025-09-30$/);
  await expect(card).toBeVisible();
  expect(await page.evaluate(() => document.activeElement?.tagName)).toBe(
    "BODY",
  );
  await expect(page).toHaveTitle("Palace");

  // Import from the day menu, filed under another day.
  await page.locator("header.day h1").hover();
  await dayMenu.click();
  await page.getByRole("menuitem", { name: "导入对话" }).click();
  const dialog = page.getByRole("dialog", { name: "导入对话" });
  await expect(
    dialog.getByRole("button", { name: "选择对话发生日期" }),
  ).toHaveText("2025 年 09 月 30 日");
  await dialog.getByLabel("标题").fill("思考的下一步");
  await dialog.getByLabel("Session ID").fill("next-step");
  await dialog.getByRole("button", { name: "选择对话发生日期" }).click();
  const day = await page.evaluate(() =>
    new Date(2025, 8, 2).toLocaleDateString(),
  );
  await page.locator(`[data-day="${day}"]`).click();
  await dialog.getByLabel("对话发生时间", { exact: true }).fill("09:30");
  await dialog.getByLabel("选择对话 JSON 文件").setInputFiles({
    name: "conversation.json",
    mimeType: "application/json",
    buffer: Buffer.from(JSON.stringify([{ role: "user", content: "你好" }])),
  });
  const uploaded = page.waitForRequest("**/api/import/file");
  await dialog.getByRole("button", { name: "导入" }).click();
  const upload = await uploaded;
  expect(upload.headers()["content-type"]).toMatch(
    /^multipart\/form-data; boundary=.+/,
  );
  expect(upload.headers()["origin"]).toBe(new URL(page.url()).origin);
  const form = await new Response(new Uint8Array(upload.postDataBuffer()!), {
    headers: { "Content-Type": upload.headers()["content-type"] },
  }).formData();
  expect([
    form.get("title"),
    form.get("session_id"),
    form.get("occurred_at"),
  ]).toEqual([
    "思考的下一步",
    "next-step",
    String(new Date(2025, 8, 2, 9, 30).getTime()),
  ]);
  await expect(page).toHaveURL(/\/conversations\/c2\?path=p2&date=2025-09-02$/);
  await expect(page.getByRole("link", { name: "← 9 月 2 日" })).toBeVisible();
  // Browser back and the back link agree: the day it was filed under, with the new card.
  await page.goBack();
  await expect(page).toHaveURL(/\/\?date=2025-09-02$/);
  await expect(page.getByRole("link", { name: /思考的下一步/ })).toBeVisible();
  await page.goForward();

  // Rename the path and correct the source from the title menu.
  await page.getByRole("heading", { level: 1 }).hover();
  await page.getByRole("button", { name: "管理对话" }).click();
  await page.getByRole("menuitem", { name: "编辑对话" }).click();
  const editor = page.getByRole("dialog", { name: "编辑对话" });
  await editor.getByLabel("标题").fill("下一步的思考");
  await editor.getByLabel("来源").selectOption("gemini");
  await editor.getByRole("button", { name: "保存" }).click();
  await expect(
    page.getByRole("heading", { level: 1, name: "下一步的思考" }),
  ).toBeVisible();
  await expect(page.locator(".meta")).toContainText("Gemini");
  expect(requests.at(-1)).toEqual({
    method: "PUT",
    path: "/api/conversations/c2/paths/p2/metadata",
    body: { title: "下一步的思考", source: "gemini" },
  });

  // Deleting returns to the day it came from, and back does not revisit the deleted page.
  await page.getByRole("heading", { level: 1 }).hover();
  await page.getByRole("button", { name: "管理对话" }).click();
  await page.getByRole("menuitem", { name: "删除对话" }).click();
  await page
    .getByRole("dialog", { name: "删除对话？" })
    .getByRole("button", { name: "删除对话" })
    .click();
  await expect(page).toHaveURL(/\/\?date=2025-09-02$/);
  await expect(page.getByText("这一天还没有记录。")).toBeVisible();

  // Narrow screens keep everything inside the width.
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/?date=2025-09-30");
  await expect(card).toBeVisible();
  await page.screenshot({ path: "/tmp/palace-mobile.png", fullPage: true });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
  expect(errors).toEqual([]);
});

// Core test case: `specs/test-cases/web/navigation/text-nav-and-day-routes.md#hover-menus-must-stay-reachable-by-keyboard-and-touch`
test("touch screens always show the menus", async ({ browser }) => {
  const context = await browser.newContext({ hasTouch: true, isMobile: true });
  const page = await context.newPage();
  await serve(page);
  await page.goto("/?date=2025-09-30");
  await expect(page.getByRole("button", { name: "当天操作" })).toHaveCSS(
    "opacity",
    "1",
  );
  await context.close();
});
