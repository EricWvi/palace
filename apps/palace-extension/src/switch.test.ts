import { describe, expect, it } from "vitest";
import { onPalace } from "./switch";

describe("onPalace", () => {
  it("matches Palace's origin and nothing beside it", () => {
    expect(
      [
        "https://palace.test/articles/a1",
        "https://palace.test:8443/articles/a1",
        "http://palace.test/articles/a1",
        "https://blog.palace.test/post",
        "https://example.com/?next=https://palace.test",
        "not a url",
      ].map((url) => onPalace(url, "https://palace.test")),
    ).toEqual([true, false, false, false, false, false]);
  });
});
