import { describe, expect, it } from "vitest";
import { groundOf, themeToken } from "./highlights";

describe("groundOf", () => {
  it("puts dark ink on a light ground and light ink on a dark one", () => {
    expect(
      [
        "rgb(31, 30, 28)",
        "rgb(230, 230, 230)",
        "rgba(255, 255, 255, 0.87)",
      ].map(groundOf),
    ).toEqual(["light", "dark", "dark"]);
  });
});

describe("themeToken", () => {
  it("reads each theme's mark colours as the prototype draws them", () => {
    expect(
      (["light", "dark"] as const).map((ground) => [
        themeToken(ground, "mark-yellow"),
        themeToken(ground, "mark-line"),
        themeToken(ground, "text"),
      ]),
    ).toEqual([
      ["#f5e3a0", themeToken("light", "accent"), themeToken("light", "text")],
      ["#5a4a18", "#e07a52", "#dcd7cc"],
    ]);
  });
});
