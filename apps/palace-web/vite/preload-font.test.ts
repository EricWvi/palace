import { expect, test } from "vitest";
import { firstPaintFontPreload } from "./preload-font";

test("preloads only the Latin regular face among the emitted fonts", () => {
  expect(
    firstPaintFontPreload([
      "assets/index-a1b2.js",
      "assets/eb-garamond-latin-ext-400-normal-Xy12.woff2",
      "assets/eb-garamond-latin-400-normal-BCNrxLz_.woff2",
      "assets/eb-garamond-latin-400-normal-maQnHeKB.woff",
      "assets/eb-garamond-latin-400-italic-DSEbMgZu.woff2",
      "assets/eb-garamond-latin-500-normal-Q9w8.woff2",
    ]),
  ).toEqual({
    tag: "link",
    attrs: {
      rel: "preload",
      as: "font",
      type: "font/woff2",
      href: "/assets/eb-garamond-latin-400-normal-BCNrxLz_.woff2",
      crossorigin: true,
    },
    injectTo: "head",
  });
});

test("fails the build when the face is missing or ambiguous", () => {
  expect(() => firstPaintFontPreload(["assets/index-a1b2.js"])).toThrow(
    /found 0/,
  );
  expect(() =>
    firstPaintFontPreload([
      "assets/eb-garamond-latin-400-normal-aaaa.woff2",
      "assets/eb-garamond-latin-400-normal-bbbb.woff2",
    ]),
  ).toThrow(/found 2/);
});
