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

  // A brief pass over the ticks must never start a translucent panel opening.
  await page.clock.install({ time: new Date("2026-10-19T00:00:00+08:00") });
  await page.clock.pauseAt(new Date("2026-10-19T00:00:01+08:00"));
  const exitTrigger = (await toc.locator(".toc-rail").boundingBox())!;
  for (const exit of [
    "viewport",
    "document",
    "pointer",
    "mouseout",
    "blur",
    "hidden",
  ] as const) {
    await page.mouse.move(
      exitTrigger.x + exitTrigger.width - 4,
      exitTrigger.y + exitTrigger.height / 2,
    );
    await page.clock.runFor(100);
    if (exit === "viewport") {
      await page.mouse.move(1300, exitTrigger.y + exitTrigger.height / 2);
    } else {
      // Model a boundary notification without the element's own leave event, including stale :hover.
      await page.evaluate((exit) => {
        switch (exit) {
          case "document":
            document.documentElement.dispatchEvent(
              new MouseEvent("mouseleave", { relatedTarget: null }),
            );
            break;
          case "pointer":
            document.documentElement.dispatchEvent(
              new PointerEvent("pointerleave"),
            );
            break;
          case "mouseout":
            window.dispatchEvent(
              new MouseEvent("mouseout", { relatedTarget: null }),
            );
            break;
          case "blur":
            window.dispatchEvent(new Event("blur"));
            break;
          case "hidden":
            Object.defineProperty(document, "hidden", {
              configurable: true,
              value: true,
            });
            document.dispatchEvent(new Event("visibilitychange"));
            break;
        }
      }, exit);
    }
    await page.clock.runFor(1000);
    await panel.evaluate((element) =>
      element.getAnimations().forEach((animation) => animation.finish()),
    );
    expect(await opacity(), exit).toBe("0");
    if (exit === "hidden") {
      await page.evaluate(() => Reflect.deleteProperty(document, "hidden"));
    }
    await page.mouse.move(400, 400);
  }
  for (const colorScheme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme });
    const trigger = (await toc.locator(".toc-rail").boundingBox())!;
    await page.mouse.move(
      trigger.x + trigger.width - 4,
      trigger.y + trigger.height / 2,
    );
    await page.clock.runFor(100);
    const briefOpacity = await panel.evaluate((element) => {
      // Seek the actual CSS transition, avoiding assertions tied to the runner's frame rate.
      for (const animation of element.getAnimations()) {
        animation.pause();
        animation.currentTime = 100;
      }
      return getComputedStyle(element).opacity;
    });
    expect(briefOpacity).toBe("0");
    // Leaving the ticks into the hidden panel's footprint must cancel opening too.
    await page.mouse.move(trigger.x - 100, trigger.y + trigger.height / 2);
    await page.clock.runFor(1000);
    expect(await toc.evaluate((element) => element.matches(":hover"))).toBe(
      false,
    );
    await expect.poll(opacity).toBe("0");

    await page.mouse.move(
      trigger.x + trigger.width - 4,
      trigger.y + trigger.height / 2,
    );
    await page.clock.runFor(300);
    expect(await opacity()).toBe("0");
    await page.clock.runFor(50);
    await expect(toc).toHaveAttribute("data-open", "true");
    await panel.evaluate((element) =>
      element.getAnimations().forEach((animation) => animation.finish()),
    );
    expect(await opacity()).toBe("1");
    await page.mouse.move(400, 400);
    await panel.evaluate((element) =>
      element.getAnimations().forEach((animation) => animation.finish()),
    );
    expect(await opacity()).toBe("0");

    // Keyboard access opens immediately without waiting for the mouse intent timer.
    const firstLink = toc.getByRole("link").first();
    await firstLink.focus();
    await panel.evaluate((element) =>
      element.getAnimations().forEach((animation) => animation.finish()),
    );
    expect(await opacity()).toBe("1");
    await firstLink.evaluate((element) => element.blur());
    await panel.evaluate((element) =>
      element.getAnimations().forEach((animation) => animation.finish()),
    );
    expect(await opacity()).toBe("0");
  }
  await page.clock.resume();

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
