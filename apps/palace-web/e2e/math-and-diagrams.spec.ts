import { expect, test, type Page } from "@playwright/test";
import type { Detail } from "../src/lib/api";

const content = [
  "质能方程 $E = mc^2$ 如下：",
  "",
  "$$",
  "\\int_0^1 x^2 \\, dx = \\frac{1}{3}",
  "$$",
  "",
  "```mermaid",
  "graph LR",
  "  A[开始] --> B[结束]",
  "```",
  "",
  "```mermaid",
  "graph LR; this is not -> mermaid ((",
  "```",
].join("\n");
const detail: Detail = {
  conversation: { owner_id: "owner", id: "math", source: "chatgpt" },
  messages: [
    {
      id: "m0",
      role: "assistant",
      content,
      toc_line: "公式",
      owner_id: "owner",
      conversation_id: "math",
      parent_message_id: null,
      created_order: 0,
    },
  ],
  paths: [
    {
      id: "p1",
      title: "公式",
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
  await page.goto("/conversations/math");
}

// Core test case: `specs/test-cases/server/conversation/markdown-content.md#formulas-and-diagrams-must-render-and-stay-out-of-annotations`
test("real mermaid draws in the page's colours in both themes and enlarges on click", async ({
  page,
}) => {
  for (const colorScheme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme });
    await open(page);
    // KaTeX's own fonts are what give a formula its width; a fallback would collapse it.
    await expect(page.locator(".katex-display .katex-html")).toBeVisible();
    const diagram = page.getByRole("button", { name: "放大图表" });
    await expect(diagram.locator("svg .node")).toHaveCount(2);
    const [fill, sunken] = await diagram.evaluate((el) => [
      getComputedStyle(el.querySelector(".node rect, .node path")!).fill,
      (() => {
        // The token as the browser resolves it, so both sides are rgb().
        const probe = document.createElement("div");
        probe.style.color = "var(--bg-sunken)";
        document.body.append(probe);
        const color = getComputedStyle(probe).color;
        probe.remove();
        return color;
      })(),
    ]);
    expect(fill).toBe(sunken);
    // The diagram that does not parse stays as its source.
    await expect(
      page.locator("pre[data-annotation-skip]", { hasText: "not -> mermaid" }),
    ).toBeVisible();
    // Mermaid's own error picture must not be left behind at the end of the page.
    await expect(page.getByText("Syntax error in text")).toHaveCount(0);
  }
  await page.getByRole("button", { name: "放大图表" }).click();
  const dialog = page.getByRole("dialog", { name: "图表" });
  await expect(dialog.locator("svg .node")).toHaveCount(2);
});
