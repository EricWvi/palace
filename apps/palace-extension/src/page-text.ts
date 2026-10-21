// The text of a web page as the extension anchors in it: every text node of the body end to
// end, as Palace joins the text nodes of an article. A selection becomes a text quote selector
// over it, and a located quote becomes a DOM range to draw.
import type { TextQuote } from "../../palace-web/src/lib/text-quote";

// Text the reader never sees as the page's words.
const skipped = "script,style,noscript,template,textarea,select,option";

// Surrounding text kept with a quote, in UTF-16 code units.
const contextUnits = 32;
// The server's limit on a quote.
const quoteBytes = 16 * 1024;

export type PageText = { nodes: Text[]; starts: number[]; text: string };

// Reads the text of `root` now; pages change, so callers read it again before each use.
export function pageText(root: Element): PageText {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
    acceptNode(node) {
      return node.parentElement?.closest(skipped)
        ? NodeFilter.FILTER_REJECT
        : NodeFilter.FILTER_ACCEPT;
    },
  });
  const nodes: Text[] = [];
  const starts: number[] = [];
  let text = "";
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    nodes.push(node as Text);
    starts.push(text.length);
    text += (node as Text).data;
  }
  return { nodes, starts, text };
}

// The offset of a boundary point in the page text: the counted text before it.
function offsetAt(page: PageText, node: Node, offset: number) {
  const index = page.nodes.indexOf(node as Text);
  if (index !== -1) return page.starts[index] + offset;
  // An element boundary: count every text node that ends before the point.
  const boundary = document.createRange();
  boundary.setStart(node, offset);
  let total = 0;
  for (const [index, text] of page.nodes.entries()) {
    if (boundary.comparePoint(text, text.length) > 0) return total;
    total = page.starts[index] + text.length;
  }
  return total;
}

// A code unit that starts a surrogate pair's second half must not open a slice, nor a first
// half end one: a lone surrogate is not valid text for the server.
const low = (unit: number) => unit >= 0xdc00 && unit <= 0xdfff;

// Turns a selection into a selector when it lies in the page's text, trimmed of whitespace at
// both ends; a selection in a field, outside `root`, blank or too long to keep is none.
export function selectionQuote(
  root: Element,
  range: Range,
): (TextQuote & { start: number; end: number }) | null {
  if (range.collapsed || !root.contains(range.commonAncestorContainer))
    return null;
  const container =
    range.commonAncestorContainer instanceof Element
      ? range.commonAncestorContainer
      : range.commonAncestorContainer.parentElement;
  if (container?.closest("input,textarea,[contenteditable]")) return null;
  const page = pageText(root);
  let start = offsetAt(page, range.startContainer, range.startOffset);
  let end = offsetAt(page, range.endContainer, range.endOffset);
  while (start < end && /\s/.test(page.text[start])) start++;
  while (end > start && /\s/.test(page.text[end - 1])) end--;
  if (start >= end) return null;
  const quote = page.text.slice(start, end);
  if (new TextEncoder().encode(quote).length > quoteBytes) return null;
  let before = Math.max(0, start - contextUnits);
  if (low(page.text.charCodeAt(before))) before++;
  let after = Math.min(page.text.length, end + contextUnits);
  if (low(page.text.charCodeAt(after))) after--;
  return {
    quote,
    prefix: page.text.slice(before, start),
    suffix: page.text.slice(end, after),
    position: start / page.text.length,
    start,
    end,
  };
}

// The DOM range covering `[start, end)` of the page text.
export function rangeOf(page: PageText, start: number, end: number): Range {
  const range = document.createRange();
  // The node holding a given offset: the last one starting at or before it.
  const locate = (offset: number, closing: boolean) => {
    let index = 0;
    for (let i = 0; i < page.nodes.length; i++) {
      const at = page.starts[i];
      // An end exactly at a node boundary stays in the earlier node.
      if (closing ? at < offset : at <= offset) index = i;
      else break;
    }
    return { node: page.nodes[index], offset: offset - page.starts[index] };
  };
  const from = locate(start, false);
  const to = locate(end, true);
  range.setStart(from.node, from.offset);
  range.setEnd(to.node, to.offset);
  return range;
}
