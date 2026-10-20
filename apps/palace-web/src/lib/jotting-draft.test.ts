import { describe, expect, it } from "vitest";
import {
  completeTag,
  parseDraft,
  suggestTags,
  tagFragment,
} from "./jotting-draft";

describe("parseDraft", () => {
  // specs/test-cases/web/echo/echo-page.md#composing-must-extract-inline-tags-and-keep-the-rest-of-the-body-intact
  it("takes out only real tags and keeps every other character as typed", () => {
    const draft = [
      "#rust 今天读完 #阅读 第三章",
      "# 标题",
      "C# 和 a.html#x 都不是标签",
      "- 列表",
      "  - 缩进  两个空格",
      "```",
      "let tag = #不是;",
      "    缩进的代码",
      "```",
      "#阅读 #rust",
    ].join("\n");
    expect(parseDraft(draft)).toEqual({
      tags: ["rust", "阅读"],
      body: [
        "今天读完 第三章",
        "# 标题",
        "C# 和 a.html#x 都不是标签",
        "- 列表",
        "  - 缩进  两个空格",
        "```",
        "let tag = #不是;",
        "    缩进的代码",
        "```",
      ].join("\n"),
    });
  });

  it("leaves an all-tag draft with an empty body", () => {
    expect(parseDraft("  #想法  \n\n")).toEqual({ tags: ["想法"], body: "" });
  });
});

describe("tag completion", () => {
  it("finds the fragment before the caret and completes it with a space", () => {
    const text = "读到 #阅 很有意思";
    const caret = "读到 #阅".length;
    expect(tagFragment(text, caret)).toBe("阅");
    expect(tagFragment(text, 2)).toBeNull();
    expect(suggestTags(["阅读", "生活", "阅", "重阅"], "阅")).toEqual([
      "阅读",
      "重阅",
    ]);
    expect(completeTag(text, caret, "阅读")).toEqual({
      text: "读到 #阅读  很有意思",
      caret: "读到 #阅读 ".length,
    });
  });
});
