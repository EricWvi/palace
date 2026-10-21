import { render } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { expect, it } from "vitest";
import {
  MarkdownContent,
  type ImagePolicy,
} from "@/components/markdown-content";
import { locate, type Mark, type Painting } from "./annotation-paint";
import { scopeTexts, selectedText } from "./annotation-dom";

const placeholder: ImagePolicy = { kind: "placeholder" };

// Every structure whose rendered text could drift between the DOM and the syntax tree: inline
// code, emphasis and links, a highlighted fence with its copy button, an image placeholder, a
// table, a loose list and a quote.
const body = [
  "开头一段，有 `inline code`、**粗体** 和 [链接](https://example.com)。",
  "",
  "```js",
  'const answer = "42";',
  "```",
  "",
  "![封面](https://example.com/a.png) 图后文字。",
  "",
  "| 列一 | 列二 |",
  "| --- | --- |",
  "| 甲 | 乙 |",
  "",
  "- 第一项",
  "",
  "- 第二项有字",
  "",
  "> 引用里的话。",
  "",
  "## 小节",
  "",
  "结尾一段。",
].join("\n");

function show(painting?: Painting) {
  return render(
    <MemoryRouter>
      <MarkdownContent
        content={body}
        images={placeholder}
        scope="article"
        painting={painting}
      />
    </MemoryRouter>,
  );
}

const scope = () => document.querySelector("[data-annotation-scope]")!;

// Selects `[start, end)` of the scope's counted text, the way a reader's drag would.
function select(start: number, end: number) {
  const range = document.createRange();
  let position = 0;
  for (const text of scopeTexts(scope())) {
    const next = position + text.length;
    if (start >= position && start <= next && range.startContainer === document)
      range.setStart(text, start - position);
    if (end >= position && end <= next) {
      range.setEnd(text, end - position);
      break;
    }
    position = next;
  }
  document.getSelection()!.removeAllRanges();
  document.getSelection()!.addRange(range);
  return selectedText(document.body);
}

function selectQuote(quote: string) {
  const text = scopeTexts(scope())
    .map((node) => node.data)
    .join("");
  const start = text.indexOf(quote);
  return select(start, start + quote.length);
}

function mark(
  anchor: { start: number; end: number; quote: string },
  id = "a",
): Mark {
  return { id, ...anchor, style: "yellow", noted: false };
}

const markText = (id: string) =>
  [...document.querySelectorAll(`mark[data-annotation="${id}"]`)]
    .map((element) => element.textContent)
    .join("");

// Selections across every structure draw exactly the quoted text, and the same selection made
// again over marks or folded blocks yields the same anchor.
// Core test case:
// - `specs/test-cases/web/stars/annotations.md#marks-must-land-on-the-text-that-was-selected`
it("draws marks where the selection was, in every state", () => {
  const quotes = [
    "inline code、粗体",
    "有 inline code、粗体 和 链接",
    'answer = "42"',
    "图后文字",
    "甲",
    "第二项有字",
    "引用里的话",
    // Across two blocks: the newline between them is rendered text too.
    "链接。\nconst answer",
  ];
  const view = show();
  const anchors = quotes.map((quote) => {
    const anchor = selectQuote(quote);
    expect(anchor?.quote).toBe(quote);
    return { start: anchor!.start, end: anchor!.end, quote };
  });
  const marks = anchors.map((anchor, index) => mark(anchor, `m${index}`));
  view.rerender(
    <MemoryRouter>
      <MarkdownContent
        content={body}
        images={placeholder}
        scope="article"
        painting={{ marks, fold: null }}
      />
    </MemoryRouter>,
  );
  for (const [index, anchor] of anchors.entries()) {
    expect(markText(`m${index}`)).toBe(anchor.quote);
    const again = select(anchor.start, anchor.end);
    expect(again && { ...again, range: null }).toEqual({
      scope: "article",
      ...anchor,
      range: null,
    });
  }
  view.rerender(
    <MemoryRouter>
      <MarkdownContent
        content={body}
        images={placeholder}
        scope="article"
        painting={{ marks: [marks[0]], fold: { expanded: new Set() } }}
      />
    </MemoryRouter>,
  );
  const folded = select(anchors[6].start, anchors[6].end);
  expect(folded?.quote).toBe("引用里的话");
});

// A mark across inline code is one element holding the whole code element, not three pieces.
// Core test case:
// - `specs/test-cases/web/stars/annotations.md#marks-must-land-on-the-text-that-was-selected`
it("lifts wholly marked inline elements into one seamless mark", () => {
  const view = show();
  const anchor = selectQuote("有 inline code、")!;
  view.rerender(
    <MemoryRouter>
      <MarkdownContent
        content={body}
        images={placeholder}
        scope="article"
        painting={{ marks: [mark(anchor)], fold: null }}
      />
    </MemoryRouter>,
  );
  const marks = document.querySelectorAll('mark[data-annotation="a"]');
  expect(marks).toHaveLength(1);
  expect(marks[0].innerHTML).toBe("有 <code>inline code</code>、");
});

// Offsets that no longer hold the quote fall back to its nearest occurrence, or to nothing.
// Core test case:
// - `specs/test-cases/web/stars/annotations.md#marks-must-land-on-the-text-that-was-selected`
it("relocates a quote to its nearest occurrence", () => {
  const text = "一二三，一二三，一二三";
  expect(locate(text, { start: 4, end: 7, quote: "一二三" })).toEqual({
    start: 4,
    end: 7,
  });
  expect(locate(text, { start: 6, end: 9, quote: "一二三" })).toEqual({
    start: 4,
    end: 7,
  });
  expect(locate(text, { start: 9, end: 12, quote: "一二三" })).toEqual({
    start: 8,
    end: 11,
  });
  expect(locate(text, { start: 0, end: 3, quote: "四五" })).toBeNull();
});

// Folding keeps marked blocks, the headings of sections that kept something and the marked
// items of a list, and puts one counted line in place of each run.
// Core test case:
// - `specs/test-cases/web/stars/annotations.md#focus-mode-must-fold-only-unmarked-blocks`
it("folds runs of unmarked blocks", () => {
  const view = show();
  const anchor = selectQuote("第二项有字")!;
  const ending = selectQuote("结尾一段")!;
  view.rerender(
    <MemoryRouter>
      <MarkdownContent
        content={body}
        images={placeholder}
        scope="article"
        painting={{
          marks: [mark(anchor), mark(ending, "b")],
          fold: { expanded: new Set() },
        }}
      />
    </MemoryRouter>,
  );
  const visible = [...scope().children]
    .filter((child) => !child.classList.contains("folded"))
    .map((child) =>
      child.classList.contains("fold-gap")
        ? `[${child.textContent}]`
        : child.tagName,
    );
  expect(visible).toEqual(["[省略 4 段]", "UL", "[省略 1 段]", "H2", "P"]);
  const items = [...scope().querySelectorAll("ul > li")]
    .filter((item) => !item.classList.contains("folded"))
    .map((item) => item.textContent?.trim());
  expect(items).toEqual(["省略 1 项", "第二项有字"]);
  const runs = [...scope().querySelectorAll<HTMLElement>(".fold-gap")].map(
    (gap) => gap.dataset.foldRun,
  );
  // Opening one run leaves the others folded.
  view.rerender(
    <MemoryRouter>
      <MarkdownContent
        content={body}
        images={placeholder}
        scope="article"
        painting={{
          marks: [mark(anchor), mark(ending, "b")],
          fold: { expanded: new Set([Number(runs[0])]) },
        }}
      />
    </MemoryRouter>,
  );
  expect(scope().querySelectorAll(".fold-gap")).toHaveLength(runs.length - 1);
});
