import { expect, test, type Page } from "@playwright/test";
import type { ListedJotting } from "../src/lib/api";

// Twelve jottings of uneven length, so the masonry columns differ in height; an upload joins
// the top of the wall.
function serve(page: Page) {
  let listed: ListedJotting[] = Array.from({ length: 12 }, (_, i) => ({
    id: `j${i}`,
    created_at: new Date(2026, 8, 20, 12).getTime() - i * 86_400_000,
    title: null,
    tags: ["生活"],
    body: Array.from(
      { length: (i % 4) + 1 },
      () => `第 ${i} 张札记的一段话。`,
    ).join("\n\n"),
    version: "1",
    pending: false,
    recall: "included",
  }));
  return page.route("**/api/**", async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    if (url.pathname === "/api/jottings" && request.method() === "POST") {
      const body = request.postDataJSON() as ListedJotting;
      listed = [{ ...body, title: null, pending: true }, ...listed];
      return route.fulfill({
        status: 201,
        json: { id: body.id, version: "1" },
      });
    }
    const own = /^\/api\/jottings\/(j\d+)$/.exec(url.pathname);
    if (own && request.method() === "DELETE") {
      listed = listed.filter((item) => item.id !== own[1]);
      return route.fulfill({ status: 204 });
    }
    if (own && request.method() === "PUT") {
      const body = request.postDataJSON() as Partial<ListedJotting>;
      listed = listed.map((item) =>
        item.id === own[1] ? { ...item, ...body, version: "2" } : item,
      );
      return route.fulfill({ json: { version: "2" } });
    }
    if (url.pathname === "/api/jottings")
      return route.fulfill({
        json: { jottings: listed, total: listed.length, next_cursor: null },
      });
    if (url.pathname === "/api/jottings/overview")
      return route.fulfill({
        json: {
          total: listed.length,
          days: listed.length,
          tags: [{ name: "生活", count: listed.length }],
          daily: [],
        },
      });
    if (url.pathname === "/api/jottings/recall")
      return route.fulfill({ json: { date: "2026-10-20", jottings: [] } });
    return route.fulfill({ status: 404, json: { error: "not_found" } });
  });
}

// Each card's column, by the card's id.
async function columns(page: Page) {
  return page
    .locator(".wall .col")
    .evaluateAll((cols) =>
      Object.fromEntries(
        cols.flatMap((col, i) =>
          [...col.querySelectorAll<HTMLElement>("[data-id]")].map((card) => [
            card.dataset.id,
            i,
          ]),
        ),
      ),
    );
}

// Core test case: `specs/test-cases/web/echo/echo-page.md#typing-right-after-n-must-keep-the-caret-in-place`
test("keeps text typed right after n in order while the composer slides open", async ({
  page,
}) => {
  await serve(page);
  await page.goto("/jottings");
  await expect(page.locator(".wall [data-id]")).toHaveCount(12);
  await page.keyboard.press("n");
  // No waiting for the drawer: the bug only showed while it was still growing.
  await page.keyboard.type("今天读完了第三章 and more 中文");
  await expect(page.getByRole("textbox", { name: "新札记" })).toHaveValue(
    "今天读完了第三章 and more 中文",
  );
});

// The search drawer is inert while shut, which jsdom ignores; only a real browser refuses focus.
test("takes text typed right after / in the search field", async ({ page }) => {
  await serve(page);
  await page.goto("/jottings");
  await expect(page.locator(".wall [data-id]")).toHaveCount(12);
  await page.keyboard.press("/");
  await page.keyboard.type("第三");
  await expect(page.getByRole("searchbox", { name: "搜索札记" })).toHaveValue(
    "第三",
  );
});

// Core test case: `specs/test-cases/web/echo/echo-page.md#composing-must-extract-inline-tags-and-keep-the-rest-of-the-body-intact`
test("puts a new jotting on top of the first column and moves no other card", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1280, height: 900 });
  await serve(page);
  await page.goto("/jottings");
  await expect(page.locator(".wall [data-id]")).toHaveCount(12);
  const before = await columns(page);
  expect(new Set(Object.values(before)).size).toBe(3);

  await page.keyboard.press("n");
  await page.keyboard.type("新的一张 #想法");
  // The card at the top of the first column, measured from the wall's top so the composer
  // closing above does not count as movement; and how visible the new card is, frame by frame.
  const top = page.locator(".wall .col").first().locator("[data-id]").first();
  const old = (await top.getAttribute("data-id"))!;
  const frames = page.evaluate(
    (id) =>
      new Promise<{ shift: number; opacity: number | null }[]>((resolve) => {
        const seen: { shift: number; opacity: number | null }[] = [];
        const started = performance.now();
        const watch = () => {
          const wall = document.querySelector(".wall")!;
          const card = wall.querySelector(`[data-id="${id}"]`)!;
          const first = wall.querySelector(".col [data-id]") as HTMLElement;
          seen.push({
            shift:
              card.getBoundingClientRect().top -
              wall.getBoundingClientRect().top,
            opacity:
              first === card ? null : Number(getComputedStyle(first).opacity),
          });
          if (performance.now() - started > 1500) return resolve(seen);
          requestAnimationFrame(watch);
        };
        requestAnimationFrame(watch);
      }),
    old,
  );
  await page.keyboard.press("Control+Enter");
  await expect(page.locator(".wall [data-id]")).toHaveCount(13);
  // The column slides down to make room rather than jumping, and the new card comes in after.
  const seen = await frames;
  const from = seen[0].shift;
  const to = seen.at(-1)!.shift;
  expect(to).toBeGreaterThan(from + 20);
  expect(
    seen.filter(({ shift }) => shift > from + 2 && shift < to - 2).length,
  ).toBeGreaterThan(3);
  const arrived = seen.filter(({ opacity }) => opacity !== null);
  expect(arrived[0].opacity).toBe(0);
  expect(arrived.some(({ opacity }) => opacity! > 0 && opacity! < 1)).toBe(
    true,
  );
  expect(arrived.at(-1)!.opacity).toBe(1);
  await expect(top).toContainText("新的一张");
  await expect(top.getByLabel("待整理")).toBeVisible();
  const after = await columns(page);
  for (const [id, column] of Object.entries(before))
    expect([id, after[id]]).toEqual([id, column]);
});

// Core test cases:
// - `specs/test-cases/web/echo/echo-page.md#deleting-a-jotting-must-take-two-clicks-on-the-same-slip`
// - `specs/test-cases/web/echo/echo-page.md#editing-a-zoomed-jotting-must-never-lose-the-draft`
test("a deleted jotting leaves its column alone after a vermilion 确认, and an edit stays within its cap", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1280, height: 900 });
  await serve(page);
  await page.goto("/jottings");
  await expect(page.locator(".wall [data-id]")).toHaveCount(12);
  const before = await columns(page);
  const target = "j4";
  const sameColumn = Object.entries(before)
    .filter(([id, column]) => id !== target && column === before[target])
    .map(([id]) => id);

  await page
    .locator(`.wall [data-id=${target}]`)
    .getByRole("button", { name: "放大" })
    .click();
  await expect(page.locator(".zoom-frame.settled")).toBeVisible();
  const slip = page.locator(".zoom-actions button").last();
  await expect(slip).toHaveText("削简");
  const armed = await slip.boundingBox();
  await slip.click();
  await expect(slip).toHaveText("确认");
  // Hover keeps it armed; the seal colour holds against the hover rule.
  await expect(slip).toHaveCSS("color", "rgb(184, 74, 36)");
  const confirm = await slip.boundingBox();
  expect(confirm?.width).toBeCloseTo(armed!.width, 0);
  // The card below it glides up into the gap rather than jumping there in one frame.
  const below = await page
    .locator(`.wall [data-id=${target}]`)
    .evaluate((card) => (card.nextElementSibling as HTMLElement).dataset.id!);
  const from = (await page.locator(`.wall [data-id=${below}]`).boundingBox())!
    .y;
  const tops = page.evaluate(
    (id) =>
      new Promise<number[]>((resolve) => {
        const seen: number[] = [];
        const started = performance.now();
        const watch = () => {
          const card = document.querySelector(`.wall [data-id="${id}"]`);
          if (card) seen.push(card.getBoundingClientRect().top);
          if (performance.now() - started > 1500) return resolve(seen);
          requestAnimationFrame(watch);
        };
        requestAnimationFrame(watch);
      }),
    below,
  );
  await slip.click();
  await expect(page.locator(".zoom-frame")).toHaveCount(0);
  await expect(page.locator(".wall [data-id]")).toHaveCount(11);
  const seen = await tops;
  const to = seen.at(-1)!;
  expect(to).toBeLessThan(from - 20);
  const between = seen.filter((top) => top < from - 2 && top > to + 2);
  expect(between.length).toBeGreaterThan(3);
  const after = await columns(page);
  // Every other card keeps its column; only the cards below it in its column move up.
  for (const [id, column] of Object.entries(before))
    if (id !== target) expect([id, after[id]]).toEqual([id, column]);
  expect(sameColumn.every((id) => after[id] === before[target])).toBe(true);

  // A body much longer than the screen: the card stays capped and scrolls inside.
  await page
    .locator(".wall [data-id=j0]")
    .getByRole("button", { name: "放大" })
    .click();
  await expect(page.locator(".zoom-frame.settled")).toBeVisible();
  await page
    .locator(".zoom-actions")
    .getByRole("button", { name: "修订" })
    .click();
  await page
    .getByRole("textbox", { name: "正文" })
    .fill(Array.from({ length: 120 }, (_, i) => `第 ${i} 行`).join("\n"));
  const zoom = page.locator(".echo-card.zoom");
  const box = await zoom.boundingBox();
  expect(box!.height).toBeLessThanOrEqual(900 * 0.8 + 1);
  expect(
    await zoom.evaluate(
      (element) => element.scrollHeight > element.clientHeight,
    ),
  ).toBe(true);
});

// Core test case: `specs/test-cases/web/echo/echo-page.md#a-wall-jotting-must-open-in-full-by-its-time-and-come-back-as-it-was`
test("the page under a zoomed jotting does not scroll, and the wall keeps its width", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1280, height: 420 });
  await serve(page);
  await page.goto("/jottings");
  await expect(page.locator(".wall [data-id]")).toHaveCount(12);
  const width = await page
    .locator(".wall")
    .evaluate((wall) => wall.clientWidth);
  await page
    .locator(".wall [data-id=j3]")
    .getByRole("button", { name: "放大" })
    .click();
  await expect(page.locator(".zoom-frame.settled")).toBeVisible();
  expect(await page.locator(".wall").evaluate((wall) => wall.clientWidth)).toBe(
    width,
  );
  const before = await page.evaluate(() => window.scrollY);
  // Over the dimmed page, and over the short card itself.
  await page.mouse.move(80, 380);
  await page.mouse.wheel(0, 600);
  const box = (await page.locator(".echo-card.zoom").boundingBox())!;
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.wheel(0, 600);
  await page.keyboard.press("PageDown");
  await page.waitForTimeout(300);
  expect(await page.evaluate(() => window.scrollY)).toBe(before);

  // Closed, the page scrolls again.
  await page.keyboard.press("Escape");
  await expect(page.locator(".zoom-frame")).toHaveCount(0);
  await page.mouse.wheel(0, 600);
  await expect
    .poll(() => page.evaluate(() => window.scrollY))
    .toBeGreaterThan(before);
});

// Serves a recall pile of `count` cards and an empty wall; completions succeed.
function servePile(page: Page, count: number) {
  return page.route("**/api/**", async (route) => {
    const path = new URL(route.request().url()).pathname;
    if (/\/done$/.test(path))
      return route.fulfill({ json: { recall: "included" } });
    const jottings = Array.from({ length: count }, (_, i) => ({
      id: `r${i}`,
      created_at: new Date(2026, 8, i + 1).getTime(),
      title: null,
      tags: [],
      body: `回响第${i}张`,
      pending: false,
      recall: "included",
      done: false,
    }));
    if (path === "/api/jottings/recall")
      return route.fulfill({ json: { date: "2026-10-20", jottings } });
    if (path === "/api/jottings/overview")
      return route.fulfill({
        json: { total: 0, days: 0, tags: [], daily: [] },
      });
    return route.fulfill({
      json: { jottings: [], total: 0, next_cursor: null },
    });
  });
}

// Core test case: `specs/test-cases/web/echo/echo-page.md#only-completing-a-recall-card-may-take-it-out-of-the-pile`
test("a completed recall card fades out at its slot and never flashes back at full size", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1280, height: 900 });
  await servePile(page, 5);
  await page.goto("/jottings");
  const cards = page
    .getByRole("region", { name: "今日回响" })
    .getByRole("button");
  await cards.first().click();
  await cards.nth(1).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  const done = page.getByRole("button", { name: "温故" });
  await expect(done).toBeVisible();

  // Watch every frame from the click that completes the card until the card is gone.
  const frames = page.evaluate(
    () =>
      new Promise<number[]>((resolve) => {
        const seen: number[] = [];
        const watch = () => {
          const zoom = document.querySelector(".zoom-frame");
          if (!zoom) return resolve(seen);
          seen.push(Number(getComputedStyle(zoom).opacity));
          requestAnimationFrame(watch);
        };
        requestAnimationFrame(watch);
      }),
  );
  await done.click();
  const opacities = await frames;
  expect(opacities.length).toBeGreaterThan(3);
  expect(opacities.at(-1)).toBeLessThan(0.05);
  await expect(cards).toHaveCount(4);
});

// Each spread card's cell, as row and column from its place on screen.
async function cells(page: Page) {
  return page.locator(".recall .rc").evaluateAll((cards) => {
    const boxes = cards.map((card) => card.getBoundingClientRect());
    const left = Math.min(...boxes.map((box) => box.left));
    const top = Math.min(...boxes.map((box) => box.top));
    return cards.map((card, i) => ({
      id: (card as HTMLElement).dataset.id,
      column: Math.round((boxes[i].left - left) / (boxes[i].width + 12)),
      row: Math.round((boxes[i].top - top) / (boxes[i].height + 12)),
      right: boxes[i].right,
    }));
  });
}

// Core test case: `specs/test-cases/web/echo/echo-page.md#the-spread-pile-must-lay-out-up-to-three-rows-of-six`
test("a pile of eighteen spreads into three rows of six and closes ranks as cards leave", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1280, height: 900 });
  await servePile(page, 18);
  await page.goto("/jottings");
  const pile = page.getByRole("region", { name: "今日回响" });
  await pile.getByRole("button").first().click();
  await expect(pile).toHaveClass(/settled/);
  await page.waitForFunction(() =>
    [...document.querySelectorAll(".rc")].every(
      (card) => card.getAnimations().length === 0,
    ),
  );
  const spread = await cells(page);
  expect(spread.map(({ id, column, row }) => ({ id, column, row }))).toEqual(
    Array.from({ length: 18 }, (_, i) => ({
      id: `r${i}`,
      column: i % 6,
      row: Math.floor(i / 6),
    })),
  );
  expect(Math.max(...spread.map((cell) => cell.right))).toBeLessThanOrEqual(
    1280,
  );

  // Reading the second card moves every later card up one cell, across rows too.
  await pile.getByRole("button").nth(1).click();
  await page.getByRole("button", { name: "温故" }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(page.locator(".recall .rc")).toHaveCount(17);
  await page.waitForFunction(() =>
    [...document.querySelectorAll(".rc")].every(
      (card) => card.getAnimations().length === 0,
    ),
  );
  const closed = await cells(page);
  expect(closed.map(({ id, column, row }) => ({ id, column, row }))).toEqual(
    Array.from({ length: 17 }, (_, i) => ({
      id: `r${i < 1 ? i : i + 1}`,
      column: i % 6,
      row: Math.floor(i / 6),
    })),
  );

  // Narrower, the rows hold as many cards as fit beside the pile.
  await page.keyboard.press("Escape");
  await page.setViewportSize({ width: 900, height: 900 });
  await pile.getByRole("button").first().click();
  await page.waitForFunction(() =>
    [...document.querySelectorAll(".rc")].every(
      (card) => card.getAnimations().length === 0,
    ),
  );
  const narrow = await cells(page);
  const columns = Math.max(...narrow.map((cell) => cell.column)) + 1;
  expect(columns).toBeLessThan(6);
  expect(Math.max(...narrow.map((cell) => cell.right))).toBeLessThanOrEqual(
    900,
  );
});

// Core test case: `specs/test-cases/web/echo/echo-page.md#only-a-jotting-read-in-full-may-load-pictures-and-only-palace-media`
test("a jotting read in full grows evenly for a late tall picture and caps its height", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1280, height: 900 });
  const media = "/api/media/01a169c8-2109-774d-a490-e8537c284595";
  let release = () => {};
  const arrived = new Promise<void>((resolve) => (release = resolve));
  await page.route("**/api/**", async (route) => {
    const path = new URL(route.request().url()).pathname;
    // A phone screenshot: far taller than wide, held back until the card has grown.
    if (path === media) {
      await arrived;
      return route.fulfill({
        contentType: "image/svg+xml",
        body: '<svg xmlns="http://www.w3.org/2000/svg" width="400" height="1600"><rect width="400" height="1600" fill="gray"/></svg>',
      });
    }
    if (path === "/api/jottings/recall")
      return route.fulfill({ json: { date: "2026-10-20", jottings: [] } });
    if (path === "/api/jottings/overview")
      return route.fulfill({
        json: { total: 1, days: 1, tags: [], daily: [] },
      });
    return route.fulfill({
      json: {
        jottings: [
          {
            id: "p1",
            created_at: new Date(2026, 8, 20, 12).getTime(),
            title: null,
            tags: [],
            body: `硬币动画\n\n![硬币](${media})\n\n之后的一段话。`,
            pending: false,
          },
        ],
        total: 1,
        next_cursor: null,
      },
    });
  });
  await page.goto("/jottings");
  const card = page.locator(".wall [data-id=p1]");
  await expect(card).toContainText("[图片：硬币]");
  await card.getByRole("button", { name: "放大" }).click();
  const dialog = page.getByRole("dialog");
  await expect(page.locator(".zoom-frame")).toHaveClass(/settled/);
  const before = (await dialog.boundingBox())!;

  release();
  const picture = dialog.getByRole("img", { name: "硬币" });
  await expect
    .poll(async () => (await picture.boundingBox())?.height ?? 0)
    .toBeGreaterThan(300);
  expect((await picture.boundingBox())!.height).toBeLessThanOrEqual(360);
  const after = (await dialog.boundingBox())!;
  expect(after.height).toBeGreaterThan(before.height + 300);
  expect(after.height).toBeLessThanOrEqual(900 * 0.8 + 1);
  // Grown the same both ways: still centred.
  expect(Math.abs(after.y + after.height / 2 - 450)).toBeLessThan(2);
  await expect(card.locator("img")).toHaveCount(0);
});

// Core test case: `specs/test-cases/web/echo/echo-page.md#only-a-jotting-read-in-full-may-load-pictures-and-only-palace-media`
test("a jotting longer than the screen never shows past its cap while it opens", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.route("**/api/**", (route) => {
    const path = new URL(route.request().url()).pathname;
    if (path === "/api/jottings/recall")
      return route.fulfill({ json: { date: "2026-10-20", jottings: [] } });
    if (path === "/api/jottings/overview")
      return route.fulfill({
        json: { total: 1, days: 1, tags: [], daily: [] },
      });
    return route.fulfill({
      json: {
        jottings: [
          {
            id: "long",
            created_at: new Date(2026, 8, 20, 12).getTime(),
            title: "很长的札记",
            tags: [],
            body: Array.from({ length: 80 }, (_, i) => `第 ${i} 行。`).join(
              "\n\n",
            ),
            pending: false,
            recall: "included",
          },
        ],
        total: 1,
        next_cursor: null,
      },
    });
  });
  await page.goto("/jottings");
  const card = page.locator(".wall [data-id=long]");
  await expect(card).toBeVisible();

  // Every frame from the click until the card has settled, and a few after.
  const heights = page.evaluate(
    () =>
      new Promise<number[]>((resolve) => {
        const seen: number[] = [];
        let after = 0;
        const watch = () => {
          const zoom = document.querySelector(".echo-card.zoom");
          if (zoom) seen.push(zoom.getBoundingClientRect().height);
          if (document.querySelector(".zoom-frame.settled")) after += 1;
          if (after > 10) return resolve(seen);
          requestAnimationFrame(watch);
        };
        requestAnimationFrame(watch);
      }),
  );
  await card.getByRole("button", { name: "放大" }).click();
  const seen = await heights;
  expect(seen.length).toBeGreaterThan(5);
  expect(Math.max(...seen)).toBeLessThanOrEqual(900 * 0.8 + 1);
});
