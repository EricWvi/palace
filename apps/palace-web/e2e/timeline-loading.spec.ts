import { expect, test, type Page } from "@playwright/test";
import type { Moment } from "../src/lib/api";

function moment(id: string, excerptLines: number): Moment {
  return {
    kind: "conversation",
    id,
    occurred_at: new Date(2025, 8, 30, 10).getTime(),
    conversation_id: `c-${id}`,
    title: `对话 ${id}`,
    source: "chatgpt",
    message_count: 2,
    excerpt: [
      { role: "user" as const, text: "这个周末想出去走走" },
      { role: "assistant" as const, text: "好主意" },
    ].slice(0, excerptLines),
  };
}

// Answers the outline at once and holds the cards until the test releases them, so the page is
// caught with its placeholders on screen.
async function serve(page: Page, outline: Moment[]) {
  let release: (moments: Moment[]) => void = () => {};
  const cards = new Promise<Moment[]>((resolve) => (release = resolve));
  await page.route("**/api/timeline/outline?*", (route) =>
    route.fulfill({ json: outline.map(({ id, kind }) => ({ id, kind })) }),
  );
  await page.route("**/api/timeline?*", async (route) =>
    route.fulfill({ json: await cards }),
  );
  return release;
}

// Records every state an item passes through, so a test can tell whether a morph ever ran.
async function watchStates(page: Page) {
  await page.evaluate(() => {
    const seen = new Set<string>();
    (window as unknown as { seen: Set<string> }).seen = seen;
    new MutationObserver((records) => {
      for (const record of records)
        for (const node of [record.target, ...record.addedNodes])
          if (node instanceof HTMLElement && node.dataset.state)
            seen.add(node.dataset.state);
    }).observe(document.body, {
      subtree: true,
      childList: true,
      attributes: true,
      attributeFilter: ["data-state"],
    });
  });
}
const seenStates = (page: Page) =>
  page.evaluate(() => [...(window as unknown as { seen: Set<string> }).seen]);
const items = (page: Page) => page.locator("ol.timeline > li");

// Core test case: `specs/test-cases/server/moment/timeline-loading.md#each-skeleton-must-become-the-card-with-its-own-id`
test("placeholders grow into their own cards, collapse or make room, and leave no trace", async ({
  page,
}) => {
  const [first, gone, added] = [
    moment("p1", 2),
    moment("p2", 2),
    moment("p3", 1),
  ];
  const release = await serve(page, [first, gone]);
  await page.goto("/?date=2025-09-30");
  await expect(items(page)).toHaveCount(2);
  await expect(items(page).first()).toHaveAttribute("data-state", "skeleton");
  await expect(items(page).first().locator(".skeleton")).toHaveCSS(
    "animation-name",
    "breathe",
  );
  await watchStates(page);
  const placeholder = await items(page).first().elementHandle();

  // p2 was deleted and p3 filed between the requests; the cards decide the final list.
  release([added, first]);
  await expect(page.getByRole("link", { name: /对话 p1/ })).toBeVisible();
  // While it morphs the departed placeholder is still collapsing in place.
  expect(await seenStates(page)).toEqual(
    expect.arrayContaining(["arriving", "leaving", "entering"]),
  );
  await expect
    .poll(() =>
      items(page).evaluateAll((all) =>
        all.map((item) => [
          (item as HTMLElement).dataset.id,
          (item as HTMLElement).dataset.state,
        ]),
      ),
    )
    .toEqual([
      ["p3", "card"],
      ["p1", "card"],
    ]);
  // The placeholder and its card are one element, and the morph left nothing behind.
  expect(
    await placeholder!.evaluate(
      (element) => element === document.getElementById("moment-p1"),
    ),
  ).toBe(true);
  expect(
    await items(page).evaluateAll((all) =>
      all.map((item) => [
        item.getAttribute("style"),
        item.getAnimations().length,
      ]),
    ),
  ).toEqual([
    [null, 0],
    [null, 0],
  ]);
  await expect(page.locator(".skeleton")).toHaveCount(0);
});

// Core test case: `specs/test-cases/server/moment/timeline-loading.md#each-skeleton-must-become-the-card-with-its-own-id`
test("asking for less motion stills the placeholders and swaps them without a morph", async ({
  page,
}) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  const release = await serve(page, [moment("p1", 2)]);
  await page.goto("/?date=2025-09-30");
  await expect(items(page).first()).toHaveAttribute("data-state", "skeleton");
  await expect(items(page).first().locator(".skeleton")).toHaveCSS(
    "animation-name",
    "none",
  );
  await watchStates(page);
  release([moment("p1", 2)]);
  await expect(page.getByRole("link", { name: /对话 p1/ })).toBeVisible();
  expect(await seenStates(page)).toEqual(["card"]);
});

// Core test case: `specs/test-cases/server/moment/timeline-loading.md#scrolling-to-a-moment-must-wait-for-card-heights-to-settle`
test("a moment named in the address is centred once the cards stop growing", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  // One-line excerpts make every card shorter than its placeholder, so an early scroll would
  // leave the target well off centre once the list shrinks.
  const day = Array.from({ length: 14 }, (_, index) => moment(`p${index}`, 1));
  const release = await serve(page, day);
  await page.goto("/?date=2025-09-30&moment=p7");
  await expect(items(page)).toHaveCount(14);
  release(day);
  await expect(page).toHaveURL(/\/\?date=2025-09-30$/);
  await expect(page.locator(".skeleton")).toHaveCount(0);
  const box = (await page.locator("#moment-p7").boundingBox())!;
  expect(Math.abs(box.y + box.height / 2 - 400)).toBeLessThan(2);
  expect(await page.evaluate(() => document.activeElement?.tagName)).toBe(
    "BODY",
  );
});
