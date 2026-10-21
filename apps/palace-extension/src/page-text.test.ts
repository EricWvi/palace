import { afterEach, describe, expect, it } from "vitest";
import { relocate } from "../../palace-web/src/lib/text-quote";
import { pageText, rangeOf, selectionQuote } from "./page-text";

afterEach(() => document.body.replaceChildren());

function page(html: string) {
  document.body.innerHTML = html;
  return document.body;
}

// The range over the first occurrence of `text` inside one text node.
function select(root: Element, text: string, from = 0, to = text.length) {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    const at = (node as Text).data.indexOf(text);
    if (at === -1) continue;
    const range = document.createRange();
    range.setStart(node, at + from);
    range.setEnd(node, at + to);
    return range;
  }
  throw new Error(`no ${text}`);
}

// Core test case:
// - `specs/test-cases/extension/web-annotations.md#relocation-must-anchor-a-web-annotation-on-the-same-words-in-the-rendered-article`
describe("page text", () => {
  it("joins the visible text nodes and leaves scripts, styles and fields out", () => {
    const root = page(
      `<h1>标题</h1>\n  <p>正文<b>加粗</b></p><script>var x = 1</script><style>p{}</style><textarea>草稿</textarea>`,
    );
    expect(pageText(root).text).toBe("标题\n  正文加粗");
  });

  it("turns a selection into a trimmed quote with its context and position", () => {
    const root = page(
      `<p>${"前".repeat(40)}</p><p>  被选中的话  </p><p>${"后".repeat(40)}</p>`,
    );
    const range = document.createRange();
    const middle = root.children[1].firstChild!;
    range.setStart(middle, 0);
    range.setEnd(middle, middle.textContent!.length);
    expect(selectionQuote(root, range)).toEqual({
      quote: "被选中的话",
      prefix: `${"前".repeat(30)}  `,
      suffix: `  ${"后".repeat(30)}`,
      position: 42 / 89,
      start: 42,
      end: 47,
    });
  });

  it("never cuts a surrogate pair at the edge of the context", () => {
    // 15 emoji and one letter: 32 code units before the quote would start inside a pair.
    const root = page(`<p>${"😀".repeat(16)}a被选中b${"😀".repeat(16)}</p>`);
    const quote = selectionQuote(root, select(root, "被选中"))!;
    expect([quote.prefix, quote.suffix]).toEqual([
      `${"😀".repeat(15)}a`,
      `b${"😀".repeat(15)}`,
    ]);
  });

  it("ignores blank selections and text being typed", () => {
    const root = page(
      `<p>字 <span> </span> 字</p><div contenteditable>正在写</div>`,
    );
    expect(selectionQuote(root, select(root, " ", 0, 1))).toBeNull();
    expect(selectionQuote(root, select(root, "正在写"))).toBeNull();
  });

  it("finds a saved quote again and maps it back onto the DOM", () => {
    const root = page(
      `<p>开头</p><ul><li>第一项</li>\n<li>第二项</li></ul><p>结尾</p>`,
    );
    const saved = selectionQuote(root, select(root, "一项"))!;
    // The page changed: a banner was added before the article.
    root.insertAdjacentHTML("afterbegin", "<div>横幅</div>");
    const now = pageText(root);
    const at = relocate(now.text, saved)!;
    expect(rangeOf(now, at.start, at.end).toString()).toBe("一项");
    const across = relocate(now.text, { ...saved, quote: "第一项\n  第二项" })!;
    expect(rangeOf(now, across.start, across.end).toString()).toBe(
      "第一项\n第二项",
    );
  });
});
