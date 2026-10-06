import { expect, test } from "@playwright/test";
import type { Detail, Message } from "../src/lib/api";

// Core test case: `specs/test-cases/server/conversation/reading-page.md#the-message-toc-must-list-every-message-of-the-current-path-by-its-opening`
test("the table of contents follows reading, keeps a jump's mark and closes when the pointer leaves", async ({
  page,
}) => {
  // Long answers and short questions, so the last messages cannot reach the reading line.
  const messages: Message[] = Array.from({ length: 12 }, (_, index) => {
    const role = index % 2 ? "assistant" : "user";
    return {
      id: `m${index}`,
      role,
      content:
        role === "user"
          ? `问题 ${index}`
          : Array.from({ length: 8 }, () => `回答 ${index} 的一段话。`).join(
              "\n\n",
            ),
      toc_line: `第 ${index} 条`,
      owner_id: "owner",
      conversation_id: "long",
      parent_message_id: index ? `m${index - 1}` : null,
      created_order: index,
    };
  });
  const detail: Detail = {
    conversation: { owner_id: "owner", id: "long", source: "chatgpt" },
    messages,
    paths: [
      {
        id: "p1",
        title: "长对话",
        session_id: "s1",
        head_message_id: "m11",
        message_count: messages.length,
        occurred_at: new Date(2025, 9, 1, 9, 30).getTime(),
        created_at: 1000,
        updated_at: 1000,
        original_link: "https://chatgpt.com/c/s1",
      },
    ],
  };
  await page.route("**/api/**", (route) => route.fulfill({ json: detail }));
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/conversations/long");

  const toc = page.getByRole("navigation", { name: "消息目录" });
  const panel = toc.locator(".toc-panel");
  const opacity = () => panel.evaluate((el) => getComputedStyle(el).opacity);
  const current = toc.locator('a[aria-current="true"]');
  await expect(toc.getByRole("link")).toHaveCount(12);
  await expect(current).toHaveText("第 0 条");
  await expect.poll(opacity).toBe("0");

  // Reading moves the mark without opening anything.
  await page.mouse.wheel(0, 900);
  await expect(current).not.toHaveText("第 0 条");

  // The list opens right over the ticks, so a locator hover would find them covered.
  const rail = (await toc.locator(".toc-rail").boundingBox())!;
  await page.mouse.move(rail.x + rail.width - 4, rail.y + rail.height / 2);
  await expect.poll(opacity).toBe("1");
  await toc.getByRole("link", { name: "第 10 条" }).click();
  await expect
    .poll(() =>
      page.evaluate(() =>
        Math.round(
          document.getElementById("message-m10")!.getBoundingClientRect().top,
        ),
      ),
    )
    .toBeLessThan(800);
  // Wait for the smooth scroll to settle at the bottom, where the old rule named the last one.
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          innerHeight + scrollY >= document.documentElement.scrollHeight - 2,
      ),
    )
    .toBe(true);
  await expect(current).toHaveText("第 10 条");

  await page.mouse.move(400, 400);
  await expect.poll(opacity).toBe("0");

  // The reader's own scroll lifts the pin.
  await page.mouse.wheel(0, -200);
  await expect(current).not.toHaveText("第 10 条");

  // Beside a narrow sheet there is no margin to hold it.
  await page.setViewportSize({ width: 820, height: 800 });
  await expect(toc).toBeHidden();
});
