import type { Element, ElementContent, Root, Text } from "hast";
import type { components } from "./generated/api";

export type AnnotationStyle = components["schemas"]["AnnotationStyle"];

// What the plugin needs of one annotation: its anchor, its look, and whether a note hangs on it.
export type Mark = {
  id: string;
  start: number;
  end: number;
  quote: string;
  style: AnnotationStyle | null;
  noted: boolean;
};

// Folding for 只看标注: runs of unmarked blocks collapse, except the runs the reader opened.
// Runs are numbered in document order within one scope.
export type Fold = { expanded: ReadonlySet<number> };

// Pass a stable value (memoized per scope) so the memoized Markdown re-renders only on change.
export type Painting = { marks: Mark[]; fold: Fold | null };

// React drops inter-element whitespace inside these, so neither side counts it.
const tableElements = new Set(["table", "thead", "tbody", "tfoot", "tr"]);
const blank = /^[ \t\n\f\r]*$/;
// An inline element whose whole text is marked moves inside the mark, so its own ground (inline
// code's tint, a link's colour) yields to the annotation instead of showing around it.
const lift = new Set(["code", "a", "strong", "em", "del"]);
const headings = /^h[1-6]$/;

export const highlightStyles = ["yellow", "green", "blue", "rose"] as const;

// How each style is drawn, by the class the stylesheet keys on; a note alone has its own look.
export function markKind(style: AnnotationStyle | null) {
  if (style === null) return "plain";
  if (style === "underline") return "line";
  if (style === "wavy") return "wave";
  return "hl";
}

type Parent = Root | Element;
type Located = { node: Text; parent: Parent };

// Whether a text node is part of the annotation text: everything React renders, which is every
// text node except whitespace between table parts.
function counted(node: Text, parent: Parent) {
  return !(
    parent.type === "element" &&
    tableElements.has(parent.tagName) &&
    blank.test(node.value)
  );
}

// Every counted text node in document order, with its parent.
function textNodes(tree: Parent): Located[] {
  const found: Located[] = [];
  const walk = (parent: Parent) => {
    for (const child of parent.children) {
      if (child.type === "text") {
        if (counted(child, parent)) found.push({ node: child, parent });
      } else if (child.type === "element") {
        walk(child);
      }
    }
  };
  walk(tree);
  return found;
}

// The annotation text of a rendered tree: every counted text node, end to end.
export function treeText(tree: Parent) {
  return textNodes(tree)
    .map(({ node }) => node.value)
    .join("");
}

function elementText(element: Element): string {
  return treeText(element);
}

// Where a stored anchor lands in this text: its own offsets when the text there is still the
// quote, otherwise the occurrence of the quote nearest to where it was, otherwise nowhere.
export function locate(
  text: string,
  { start, end, quote }: Pick<Mark, "start" | "end" | "quote">,
): { start: number; end: number } | null {
  if (text.slice(start, end) === quote) return { start, end };
  let best: number | null = null;
  for (
    let at = text.indexOf(quote);
    at !== -1;
    at = text.indexOf(quote, at + 1)
  ) {
    if (best === null || Math.abs(at - start) < Math.abs(best - start))
      best = at;
  }
  return best === null ? null : { start: best, end: best + quote.length };
}

// The chain of parents from the tree down to a node, needed to lift a wholly marked element.
function ancestry(tree: Root, target: Text): Parent[] {
  const path: Parent[] = [];
  const walk = (parent: Parent): boolean => {
    path.push(parent);
    for (const child of parent.children) {
      if (child === target) return true;
      if (child.type === "element" && walk(child)) return true;
    }
    path.pop();
    return false;
  };
  walk(tree);
  return path;
}

function markElement(mark: Mark): Element {
  const properties: Element["properties"] = {
    className: ["anno", markKind(mark.style)],
    dataAnnotation: mark.id,
  };
  if (mark.style) properties.dataStyle = mark.style;
  if (mark.noted) properties.dataNote = "";
  return { type: "element", tagName: "mark", properties, children: [] };
}

// Wraps every text run of `range` in the mark's element. Earlier marks end up outside later ones,
// since a later mark splits text that may already sit inside an earlier one.
function paint(tree: Root, mark: Mark, range: { start: number; end: number }) {
  let position = 0;
  const runs: { node: Text; from: number; to: number }[] = [];
  for (const { node } of textNodes(tree)) {
    const from = Math.max(range.start, position);
    const to = Math.min(range.end, position + node.value.length);
    if (from < to)
      runs.push({ node, from: from - position, to: to - position });
    position += node.value.length;
  }
  const made: Element[] = [];
  for (const { node, from, to } of runs) {
    const path = ancestry(tree, node);
    const parent = path[path.length - 1];
    const index = parent.children.indexOf(node);
    // Split into the text before, the marked text and the text after.
    const pieces: Text[] = [];
    if (from > 0)
      pieces.push({ type: "text", value: node.value.slice(0, from) });
    const middle: Text = { type: "text", value: node.value.slice(from, to) };
    pieces.push(middle);
    if (to < node.value.length)
      pieces.push({ type: "text", value: node.value.slice(to) });
    parent.children.splice(index, 1, ...pieces);
    // Climb while the whole parent is this run's text, then wrap what was reached.
    let target: ElementContent = middle;
    let holder: Parent = parent;
    for (let depth = path.length - 1; depth > 0; depth--) {
      const element = path[depth] as Element;
      if (!lift.has(element.tagName) || elementText(element) !== middle.value)
        break;
      target = element;
      holder = path[depth - 1];
    }
    const at = holder.children.indexOf(target);
    // Neighbouring runs share one mark, so a stroke across inline code has no seam.
    const previous = holder.children[at - 1];
    if (previous && previous === made[made.length - 1]) {
      holder.children.splice(at, 1);
      previous.children.push(target);
      continue;
    }
    const element = markElement(mark);
    element.children.push(target);
    holder.children.splice(at, 1, element);
    made.push(element);
  }
  // A styled mark that also carries a note ends in a small dot, drawn by the last run.
  const last = made[made.length - 1];
  if (last && mark.noted && mark.style) last.properties.dataTail = "";
}

function contains(node: ElementContent, tagName: string): boolean {
  return (
    node.type === "element" &&
    (node.tagName === tagName ||
      node.children.some((child) => contains(child, tagName)))
  );
}

function addClass(element: Element, name: string) {
  const current = element.properties.className;
  element.properties.className = [
    ...(Array.isArray(current) ? current : current ? [String(current)] : []),
    name,
  ];
}

// Folds runs of unmarked blocks among `blocks`, numbering each run from `next`. A heading stays
// when its section kept something, so what remains keeps its outline; a kept list folds its own
// unmarked items. Returns the next free run number.
function foldBlocks(
  parent: Parent,
  noun: string,
  fold: Fold,
  next: number,
): number {
  // Root and element children differ only in doctype nodes, which rendered Markdown never has.
  const blocks = (parent.children as ElementContent[]).filter(
    (child): child is Element => child.type === "element",
  );
  const marked = (block: Element) => contains(block, "mark");
  const keep = new Set(blocks.filter(marked));
  blocks.forEach((block, index) => {
    if (!headings.test(block.tagName)) return;
    for (
      let later = index + 1;
      later < blocks.length && !headings.test(blocks[later].tagName);
      later++
    ) {
      if (keep.has(blocks[later])) {
        keep.add(block);
        return;
      }
    }
  });
  const runs: Element[][] = [];
  let run: Element[] = [];
  for (const block of blocks) {
    if (keep.has(block)) {
      if (run.length) runs.push(run);
      run = [];
      if (block.tagName === "ul" || block.tagName === "ol")
        next = foldBlocks(block, "项", fold, next);
    } else {
      run.push(block);
    }
  }
  if (run.length) runs.push(run);
  for (const hidden of runs) {
    const number = next++;
    if (fold.expanded.has(number)) continue;
    const list = parent.type === "element" && /^(ul|ol)$/.test(parent.tagName);
    const gap: Element = {
      type: "element",
      tagName: list ? "li" : "div",
      properties: {
        className: ["fold-gap"],
        dataAnnotationSkip: "",
        dataFoldRun: String(number),
        title: "展开",
      },
      children: [{ type: "text", value: `省略 ${hidden.length} ${noun}` }],
    };
    for (const block of hidden) addClass(block, "folded");
    parent.children.splice(parent.children.indexOf(hidden[0]), 0, gap);
  }
  return next;
}

// The rehype plugin: draws every mark that can be located, then folds the unmarked blocks when
// 只看标注 is on. It runs after highlighting, which only wraps text and never changes it.
export function rehypeAnnotations(painting: Painting) {
  return (tree: Root) => {
    const text = treeText(tree);
    for (const mark of painting.marks) {
      const range = locate(text, mark);
      if (range) paint(tree, mark, range);
    }
    if (painting.fold) foldBlocks(tree, "段", painting.fold, 0);
  };
}
