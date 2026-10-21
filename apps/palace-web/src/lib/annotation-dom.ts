// The browser side of the annotation text (see annotation-paint.ts): the same text counted on the
// rendered DOM, so a selection turns into the offsets the plugin draws at.

const tableElements = new Set(["TABLE", "THEAD", "TBODY", "TFOOT", "TR"]);
const blank = /^[ \t\n\f\r]*$/;

// The element that holds one scope's annotation text: an article body or one message.
export const scopeSelector = "[data-annotation-scope]";

// Every text node the plugin counts: interface text marked `data-annotation-skip` (the copy
// button, image placeholders, fold lines) and whitespace React drops between table parts are not.
export function scopeTexts(root: Element): Text[] {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
    acceptNode(node) {
      const parent = node.parentElement;
      if (parent?.closest("[data-annotation-skip]"))
        return NodeFilter.FILTER_REJECT;
      if (
        parent &&
        tableElements.has(parent.tagName) &&
        blank.test((node as Text).data)
      )
        return NodeFilter.FILTER_REJECT;
      return NodeFilter.FILTER_ACCEPT;
    },
  });
  const found: Text[] = [];
  for (let node = walker.nextNode(); node; node = walker.nextNode())
    found.push(node as Text);
  return found;
}

// The offset of a boundary point within a scope: the counted text before it.
function offsetAt(texts: Text[], node: Node, offset: number) {
  const boundary = document.createRange();
  boundary.setStart(node, offset);
  let total = 0;
  for (const text of texts) {
    if (text === node) return total + offset;
    // A text node wholly before the boundary counts in full; the first one after it ends the sum.
    if (boundary.comparePoint(text, text.length) > 0) return total;
    total += text.length;
  }
  return total;
}

export type SelectedText = {
  scope: string;
  start: number;
  end: number;
  quote: string;
  // The selection itself, for placing the bar above it.
  range: Range;
};

// Turns the current selection into an anchor, trimmed of surrounding whitespace, when it lies
// within one scope inside `area`; a selection across two messages is not an annotation.
export function selectedText(area: Element): SelectedText | null {
  const selection = document.getSelection();
  if (!selection || selection.rangeCount === 0 || selection.isCollapsed)
    return null;
  const range = selection.getRangeAt(0);
  const scopeOf = (node: Node) =>
    (node instanceof Element ? node : node.parentElement)?.closest(
      scopeSelector,
    ) ?? null;
  const root = scopeOf(range.startContainer);
  if (!root || root !== scopeOf(range.endContainer) || !area.contains(root))
    return null;
  const texts = scopeTexts(root);
  const text = texts.map((node) => node.data).join("");
  let start = offsetAt(texts, range.startContainer, range.startOffset);
  let end = offsetAt(texts, range.endContainer, range.endOffset);
  while (start < end && /\s/.test(text[start])) start++;
  while (end > start && /\s/.test(text[end - 1])) end--;
  if (start >= end) return null;
  return {
    scope: (root as HTMLElement).dataset.annotationScope!,
    start,
    end,
    quote: text.slice(start, end),
    range,
  };
}
