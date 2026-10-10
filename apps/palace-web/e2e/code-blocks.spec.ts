import { expect, test, type Page } from "@playwright/test";
import type { Detail } from "../src/lib/api";

const content = [
  "```js",
  `// ${"a long comment that forces the block to scroll sideways ".repeat(4)}`,
  'function greet(name) { return "hi " + name + 42; }',
  "```",
  "",
  "```",
  "plain output",
  "```",
].join("\n");
const detail: Detail = {
  conversation: { owner_id: "owner", id: "code", source: "chatgpt" },
  messages: [
    {
      id: "m0",
      role: "assistant",
      content,
      toc_line: "代码",
      owner_id: "owner",
      conversation_id: "code",
      parent_message_id: null,
      created_order: 0,
    },
  ],
  paths: [
    {
      id: "p1",
      title: "代码",
      session_id: "s1",
      head_message_id: "m0",
      message_count: 1,
      occurred_at: new Date(2025, 9, 1, 9, 30).getTime(),
      created_at: 1000,
      updated_at: 1000,
      original_link: "https://chatgpt.com/c/s1",
    },
  ],
};
async function open(page: Page) {
  await page.route("**/api/**", (route) => route.fulfill({ json: detail }));
  await page.goto("/conversations/code");
  await expect(page.locator(".code-block")).toHaveCount(2);
}
const opacity = (page: Page, index: number) =>
  page
    .locator(".copy-code")
    .nth(index)
    .evaluate((el) => getComputedStyle(el).opacity);

// Core test case: `specs/test-cases/server/conversation/markdown-content.md#syntax-colors-must-come-from-the-five-code-tokens-in-both-themes`
test("syntax colours come from the code tokens and stay readable in both themes", async ({
  page,
}) => {
  for (const colorScheme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme });
    await open(page);
    await page.locator(".code-block").first().hover();
    await page
      .locator(".markdown")
      .screenshot({ path: `/tmp/palace-code-${colorScheme}.png` });
    const colours = await page.evaluate(() => {
      const root = getComputedStyle(document.documentElement);
      // Resolve a token through the same engine that paints the spans.
      const paint = (value: string) => {
        const probe = document.createElement("span");
        probe.style.color = value;
        document.body.append(probe);
        const colour = getComputedStyle(probe).color;
        probe.remove();
        return colour;
      };
      const luminance = (colour: string) => {
        const [r, g, b] = colour
          .match(/\d+/g)!
          .slice(0, 3)
          .map((channel) => {
            const c = Number(channel) / 255;
            return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
          });
        return 0.2126 * r + 0.7152 * g + 0.0722 * b;
      };
      const ground = luminance(paint(root.getPropertyValue("--bg-sunken")));
      const of = (selector: string) =>
        getComputedStyle(document.querySelector(`.markdown ${selector}`)!);
      return Object.fromEntries(
        (
          [
            ["keyword", ".hljs-keyword"],
            ["string", ".hljs-string"],
            ["comment", ".hljs-comment"],
            ["literal", ".hljs-number"],
            ["name", ".hljs-title"],
          ] as const
        ).map(([token, selector]) => {
          const expected = paint(root.getPropertyValue(`--code-${token}`));
          const style = of(selector);
          const lum = luminance(expected);
          const contrast =
            (Math.max(lum, ground) + 0.05) / (Math.min(lum, ground) + 0.05);
          return [
            token,
            {
              matches: style.color === expected,
              plain:
                style.fontStyle === "normal" &&
                Number(style.fontWeight) <= 400 &&
                style.backgroundColor === "rgba(0, 0, 0, 0)",
              readable: contrast >= 4.5,
            },
          ];
        }),
      );
    });
    const ok = { matches: true, plain: true, readable: true };
    expect(colours, colorScheme).toEqual({
      keyword: ok,
      string: ok,
      comment: ok,
      literal: ok,
      name: ok,
    });
  }
});

// Core test case: `specs/test-cases/server/conversation/markdown-content.md#the-copy-button-must-copy-the-original-code-and-stay-out-of-sight-until-wanted`
test("the copy button appears on hover or keyboard focus and holds its corner while code scrolls", async ({
  page,
  browser,
}) => {
  await page.setViewportSize({ width: 900, height: 800 });
  await open(page);
  await page.mouse.move(0, 0);
  await expect.poll(() => opacity(page, 0)).toBe("0");
  await page.locator(".code-block").first().hover();
  await expect.poll(() => opacity(page, 0)).toBe("1");
  await expect.poll(() => opacity(page, 1)).toBe("0");
  await page.mouse.move(0, 0);
  await expect.poll(() => opacity(page, 0)).toBe("0");

  // Keyboard users reach it by tabbing; it shows while it holds focus.
  const button = page.getByRole("button", { name: "复制代码" }).first();
  for (let step = 0; step < 20; step += 1) {
    if (await button.evaluate((el) => el === document.activeElement)) break;
    await page.keyboard.press("Tab");
  }
  await expect(button).toBeFocused();
  await expect.poll(() => opacity(page, 0)).toBe("1");

  const before = await button.boundingBox();
  const scrolled = await page
    .locator(".code-block pre")
    .first()
    .evaluate((pre) => {
      pre.scrollLeft = 200;
      return pre.scrollLeft;
    });
  expect(scrolled).toBeGreaterThan(0);
  expect(await button.boundingBox()).toEqual(before);
  const block = (await page.locator(".code-block").first().boundingBox())!;
  expect(before!.x + before!.width <= block.x + block.width).toBe(true);
  expect(before!.y >= block.y).toBe(true);

  // Without hover the button could never be reached, so it stays, faintly.
  const touch = await browser.newContext({
    baseURL: test.info().project.use.baseURL,
    hasTouch: true,
    isMobile: true,
    viewport: { width: 390, height: 844 },
  });
  const phone = await touch.newPage();
  await open(phone);
  await expect.poll(() => opacity(phone, 0)).toBe("0.6");
  await expect.poll(() => opacity(phone, 1)).toBe("0.6");
  await touch.close();
});
