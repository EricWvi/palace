import { expect, test } from "@playwright/test";

// Core test case: specs/test-cases/web/stars/notes.md#note-toc-must-follow-rendered-headings-and-distinguish-their-levels
test("note headings share the reading rail, with shorter ticks and deeper indentation at each level", async ({
  page,
}) => {
  const content = Array.from({ length: 6 }, (_, index) =>
    [
      `${"#".repeat(index + 1)} 第 ${index + 1} 级标题`,
      Array.from(
        { length: 10 },
        () => "这是一段用于验证阅读位置与目录跳转的笔记正文。",
      ).join("\n\n"),
    ].join("\n\n"),
  ).join("\n\n");
  await page.route("**/api/**", (route) =>
    route.fulfill({
      json: {
        id: "outline",
        title: "元数据标题",
        occurred_at: new Date(2026, 9, 20, 12).getTime(),
        content,
      },
    }),
  );
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/notes/outline");
  const toc = page.getByRole("navigation", { name: "笔记目录" });
  const panel = toc.locator(".toc-panel");
  const opacity = () =>
    panel.evaluate((element) => getComputedStyle(element).opacity);
  await expect(toc.getByRole("link")).toHaveCount(6);
  await expect(toc.locator('[aria-current="true"]')).toHaveText("第 1 级标题");
  await expect.poll(opacity).toBe("0");

  expect(
    await toc
      .locator(".toc-rail span")
      .evaluateAll((elements) =>
        elements.map((element) => getComputedStyle(element).width),
      ),
  ).toEqual(["22px", "19px", "16px", "13px", "10px", "7px"]);
  expect(
    await toc
      .getByRole("link")
      .evaluateAll((elements) =>
        elements.map((element) => getComputedStyle(element).paddingLeft),
      ),
  ).toEqual(["10px", "22px", "34px", "46px", "58px", "70px"]);

  await page.mouse.wheel(0, 900);
  await expect(toc.locator('[aria-current="true"]')).not.toHaveText(
    "第 1 级标题",
  );
  for (const colorScheme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme });
    const rail = (await toc.locator(".toc-rail").boundingBox())!;
    await page.mouse.move(rail.x + rail.width - 4, rail.y + rail.height / 2);
    await expect.poll(opacity).toBe("1");
    await page.screenshot({ path: `/tmp/palace-note-toc-${colorScheme}.png` });
    await page.mouse.move(400, 400);
    await expect.poll(opacity).toBe("0");
  }
  const rail = (await toc.locator(".toc-rail").boundingBox())!;
  await page.mouse.move(rail.x + rail.width - 4, rail.y + rail.height / 2);
  await expect.poll(opacity).toBe("1");
  await toc.getByRole("link", { name: "第 5 级标题" }).click();
  await expect(toc.locator('[aria-current="true"]')).toHaveText("第 5 级标题");
  await expect
    .poll(() =>
      page
        .locator("#note-heading-4")
        .evaluate((element) => Math.round(element.getBoundingClientRect().top)),
    )
    .toBeLessThan(800);
  expect(page.url()).not.toContain("#");
  await page.mouse.move(400, 400);
  await expect.poll(opacity).toBe("0");
  await page.setViewportSize({ width: 820, height: 800 });
  await expect(toc).toBeHidden();
});
