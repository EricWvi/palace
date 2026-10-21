import { describe, expect, it } from "vitest";
import { findQuote, relocate, squeeze, type TextQuote } from "./text-quote";

const selector = (
  quote: string,
  prefix = "",
  suffix = "",
  position = 0,
): TextQuote => ({ quote, prefix, suffix, position });

// Core test case:
// - `specs/test-cases/extension/web-annotations.md#relocation-must-anchor-a-web-annotation-on-the-same-words-in-the-rendered-article`
describe("relocate", () => {
  it("keeps the original index of every character it keeps", () => {
    expect(squeeze(" a\n b😀 ")).toEqual({
      text: "ab😀",
      origin: [1, 4, 5, 6],
    });
  });

  // Palace joins blocks directly; the page put newlines and indentation between them.
  it.each([
    {
      name: "across paragraphs",
      text: "第一段结尾。第二段开头",
      quote: "结尾。\n\n    第二段",
      expected: { start: 3, end: 9, quote: "结尾。第二段" },
    },
    {
      name: "across list items and inline code",
      text: "前言一项 code 二项",
      quote: "一项\n  code\n  二项",
      expected: { start: 2, end: 12, quote: "一项 code 二项" },
    },
    {
      name: "with whitespace only inside Palace's text",
      text: "say  hello world",
      quote: "sayhello",
      expected: { start: 0, end: 10, quote: "say  hello" },
    },
    {
      name: "with surrogate pairs at both ends",
      text: "前😀中😀后",
      quote: "😀中😀",
      expected: { start: 1, end: 6, quote: "😀中😀" },
    },
  ])("finds a quote $name", ({ text, quote, expected }) => {
    expect(relocate(text, selector(quote))).toEqual(expected);
  });

  const repeated = "开头。同一句话。中间的文字。同一句话。结尾的文字。";
  it.each([
    {
      name: "the occurrence whose prefix agrees",
      chosen: selector("同一句话", "中间的文字。", "", 0),
      start: 14,
    },
    {
      name: "the occurrence whose suffix agrees",
      chosen: selector("同一句话", "", "。中间", 1),
      start: 3,
    },
    {
      name: "the longer agreement over a nearer position",
      chosen: selector("同一句话", "开头。", "。", 1),
      start: 3,
    },
    {
      name: "the nearest position when context agrees equally",
      chosen: selector("同一句话", "完全无关", "毫不相干", 0.9),
      start: 14,
    },
  ])("picks $name", ({ chosen, start }) => {
    expect(relocate(repeated, chosen)).toEqual({
      start,
      end: start + 4,
      quote: "同一句话",
    });
  });

  it.each([
    { name: "a quote the text does not have", quote: "别的句子" },
    { name: "a quote that differs by one character", quote: "同一句活" },
    { name: "a blank quote", quote: " \n" },
  ])("reports $name as not found", ({ quote }) => {
    expect(relocate(repeated, selector(quote))).toBeNull();
  });
});

describe("findQuote", () => {
  it("picks the occurrence nearest the position", () => {
    const text = "甲乙。甲乙。甲乙。";
    expect(findQuote(text, "甲乙", 0.7)).toEqual({
      start: 6,
      end: 8,
      quote: "甲乙",
    });
    expect(findQuote(text, "丙", 0.7)).toBeNull();
  });
});
