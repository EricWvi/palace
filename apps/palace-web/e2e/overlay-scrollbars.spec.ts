import { expect, test, type Locator, type Page } from "@playwright/test";

// Headless Chromium hides scrollbars by default; without that flag it draws classic ones that take
// width, as Windows does.
test.use({ launchOptions: { ignoreDefaultArgs: ["--hide-scrollbars"] } });

// A note long enough to scroll, with enough headings that the rail's panel scrolls too.
async function openLongNote(page: Page) {
  const content = Array.from({ length: 40 }, (_, index) =>
    [
      `## 第 ${index + 1} 节`,
      Array.from({ length: 4 }, () => "用来把页面撑到需要滚动的正文。").join(
        "\n\n",
      ),
    ].join("\n\n"),
  ).join("\n\n");
  await page.route("**/api/**", (route) =>
    route.fulfill({
      json: {
        id: "long",
        title: "长笔记",
        occurred_at: new Date(2026, 9, 20, 12).getTime(),
        content,
      },
    }),
  );
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/notes/long");
  await expect(page.getByRole("heading", { name: "第 40 节" })).toBeAttached();
}

const opacity = (thumb: Locator) => () =>
  thumb.evaluate((element) => getComputedStyle(element).opacity);

async function drag(page: Page, thumb: Locator, by: number) {
  const box = (await thumb.boundingBox())!;
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2 + by, {
    steps: 5,
  });
  await page.mouse.up();
}

test("a thin thumb replaces the page scrollbar, shows while scrolling and drags the page", async ({
  page,
}) => {
  await openLongNote(page);
  // No native scrollbar takes width from the page any more.
  await expect(page.locator("html")).toHaveAttribute("data-scrollbar", "thin");
  expect(
    await page.evaluate(
      () => window.innerWidth - document.documentElement.clientWidth,
    ),
  ).toBe(0);

  await page.mouse.move(400, 400);
  await page.mouse.wheel(0, 600);
  const thumb = page.locator(".overlay-scrollbar");
  await expect(thumb).toHaveCount(1);
  await expect.poll(opacity(thumb)).toBe("0.5");
  await expect.poll(opacity(thumb), { timeout: 3000 }).toBe("0");

  await page.mouse.wheel(0, 200);
  await expect.poll(opacity(thumb)).toBe("0.5");
  const before = await page.evaluate(() => window.scrollY);
  await drag(page, thumb, 100);
  expect(await page.evaluate(() => window.scrollY)).toBeGreaterThan(before);
});

test("the rail's panel gets its own thumb, and dragging it neither closes the rail nor follows a link", async ({
  page,
}) => {
  await openLongNote(page);
  const toc = page.getByRole("navigation", { name: "笔记目录" });
  const panel = toc.locator(".toc-panel");
  const rail = (await toc.locator(".toc-rail").boundingBox())!;
  await page.mouse.move(rail.x + rail.width - 4, rail.y + rail.height / 2);
  await expect.poll(opacity(panel)).toBe("1");
  const current = await toc.locator('[aria-current="true"]').textContent();

  const box = (await panel.boundingBox())!;
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.wheel(0, 60);
  const thumb = page.locator(".overlay-scrollbar[data-visible]");
  await expect(thumb).toHaveCount(1);
  // The thumb hangs inside the panel's right edge, not the window's.
  const placed = (await thumb.boundingBox())!;
  expect(placed.x + placed.width).toBeLessThan(box.x + box.width);
  expect(placed.x).toBeGreaterThan(box.x + box.width - 16);

  const scrolled = await panel.evaluate((element) => element.scrollTop);
  const pageY = await page.evaluate(() => window.scrollY);
  await drag(page, thumb, 80);
  expect(await panel.evaluate((element) => element.scrollTop)).toBeGreaterThan(
    scrolled,
  );
  await expect.poll(opacity(panel)).toBe("1");
  expect(await page.evaluate(() => window.scrollY)).toBe(pageY);
  await expect(toc.locator('[aria-current="true"]')).toHaveText(current!);
});

test("dragging a dialog's thumb scrolls it without dismissing it", async ({
  page,
}) => {
  await openLongNote(page);
  // A short window makes the shortcut panel scroll inside its dialog.
  await page.setViewportSize({ width: 1280, height: 150 });
  await page.keyboard.press("?");
  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();
  const box = (await dialog.boundingBox())!;
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.wheel(0, 40);
  const thumb = page.locator(".overlay-scrollbar[data-visible]");
  await expect(thumb).toHaveCount(1);

  const scrolled = await dialog.evaluate((element) => element.scrollTop);
  await drag(page, thumb, 40);
  expect(await dialog.evaluate((element) => element.scrollTop)).toBeGreaterThan(
    scrolled,
  );
  await expect(dialog).toBeVisible();
});
