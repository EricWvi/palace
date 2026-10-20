import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useInfiniteQuery } from "@tanstack/react-query";
import { jottingListOptions, type ListedJotting } from "@/lib/api";
import { ErrorState } from "@/components/error-state";
import { JottingCard } from "./jotting-card";

const CARD_MIN = 260;
const GAP = 16;

// Which column holds each jotting, for one filter and one column count. Placement only grows:
// a card once placed keeps its column while pages load and new jottings arrive on top.
type Placement = { key: string; columns: string[][] };

// The wall: every jotting matching the filters in newest-first columns. Each card goes to the
// shortest column at the time it arrives, so the wall reads newest first, left to right; a
// jotting newer than everything shown goes to the top of the first column instead.
export function JottingWall({
  tag,
  q,
  onTag,
}: {
  tag: string;
  q: string;
  onTag: (tag: string) => void;
}) {
  const list = useInfiniteQuery(jottingListOptions(tag, q));
  const items = list.data?.pages.flatMap((page) => page.jottings) ?? [];
  const byId = new Map(items.map((item) => [item.id, item]));

  // A state, not a ref: the wall mounts only once the first page arrives, and the width and
  // highlight effects must run again when it does.
  const [wall, setWall] = useState<HTMLDivElement | null>(null);
  const width = useWidth(wall);
  const count = columnCount(width);
  const key = `${tag}\n${q}\n${count}`;
  // Nothing is placed until the first page is measured.
  const [placement, setPlacement] = useState<Placement>({
    key: "",
    columns: [],
  });
  const columns =
    placement.key === key
      ? placement.columns
      : Array.from({ length: count }, () => []);
  const placed = new Set(columns.flat());
  const unplaced = items.filter((item) => !placed.has(item.id));

  // Unplaced cards render once in a hidden column of the real width to be measured, then move
  // into their columns before the browser paints.
  const staging = useRef<HTMLDivElement>(null);
  const columnElements = useRef<(HTMLDivElement | null)[]>([]);
  const pendingIds = unplaced.map((item) => item.id).join(" ");
  useLayoutEffect(() => {
    if (!pendingIds) return;
    const next = columns.map((column) => [...column]);
    const heights = next.map(
      (_, i) => columnElements.current[i]?.offsetHeight ?? 0,
    );
    const measured = (id: string) =>
      (staging.current?.querySelector<HTMLElement>(`[data-id="${id}"]`)
        ?.offsetHeight ?? 0) + GAP;
    const ids = pendingIds.split(" ");
    // Newer than every placed card: they belong above it, at the top of the first column.
    const firstPlaced = items.findIndex((item) => placed.has(item.id));
    const newer = firstPlaced > 0 ? ids.slice(0, firstPlaced) : [];
    next[0].unshift(...newer);
    heights[0] += newer.reduce((sum, id) => sum + measured(id), 0);
    for (const id of ids.slice(newer.length)) {
      const shortest = heights.indexOf(Math.min(...heights));
      next[shortest].push(id);
      heights[shortest] += measured(id);
    }
    // Measuring the DOM and then placing is exactly what a layout effect is for; it runs before
    // paint, so the staged cards are never seen.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setPlacement({ key, columns: next });
    // Placement reacts to which cards are waiting, not to every render's fresh arrays.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pendingIds, key]);

  // The next page loads as the end of the wall comes into view.
  const end = useRef<HTMLDivElement>(null);
  const { hasNextPage, isFetchingNextPage, fetchNextPage } = list;
  useEffect(() => {
    const target = end.current;
    if (!target || !hasNextPage || typeof IntersectionObserver === "undefined")
      return;
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting) && !isFetchingNextPage)
        void fetchNextPage();
    });
    observer.observe(target);
    return () => observer.disconnect();
  }, [hasNextPage, isFetchingNextPage, fetchNextPage]);

  useSearchHighlight(wall, q, list.data);

  if (list.isPending)
    return (
      <p role="status" className="sr-only">
        正在加载札记…
      </p>
    );
  if (list.isError)
    return <ErrorState error={list.error} retry={() => void list.refetch()} />;
  if (items.length === 0)
    // An empty library shows nothing; only a filter that misses says so.
    return tag || q ? <p className="empty">没有找到。</p> : null;

  const card = (item: ListedJotting) => (
    <JottingCard key={item.id} jotting={item} activeTag={tag} onTag={onTag} />
  );
  return (
    <>
      <div ref={setWall} className="wall">
        {columns.map((ids, i) => (
          <div
            key={i}
            className="col"
            ref={(element) => {
              columnElements.current[i] = element;
            }}
          >
            {ids.flatMap((id) => {
              const item = byId.get(id);
              return item ? [card(item)] : [];
            })}
          </div>
        ))}
      </div>
      {unplaced.length > 0 && (
        <div
          ref={staging}
          className="col wall-staging"
          aria-hidden="true"
          style={{
            width: width ? (width - GAP * (count - 1)) / count : undefined,
          }}
        >
          {unplaced.map(card)}
        </div>
      )}
      <div ref={end} />
    </>
  );
}

// As many columns of at least CARD_MIN as fit; one on a phone.
function columnCount(width: number) {
  if (!width || window.matchMedia?.("(max-width: 720px)").matches) return 1;
  return Math.max(1, Math.floor((width + GAP) / (CARD_MIN + GAP)));
}

// The element's content width, following resizes. A ResizeObserver reports once on observing,
// so the first width arrives the same way as every later one; without one there is no layout
// to measure and the width stays 0.
function useWidth(element: HTMLElement | null) {
  const [width, setWidth] = useState(0);
  useLayoutEffect(() => {
    if (!element || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(() => setWidth(element.clientWidth));
    observer.observe(element);
    return () => observer.disconnect();
  }, [element]);
  return width;
}

// Marks the search term in the cards with the CSS Custom Highlight API: the rendered Markdown is
// never rewritten, so a highlight can never split its structure or fight React over the DOM.
function useSearchHighlight(
  element: HTMLElement | null,
  q: string,
  // Only a change of the loaded pages can change the text to mark.
  pages: unknown,
) {
  useEffect(() => {
    if (!q || !element || typeof Highlight === "undefined" || !CSS.highlights)
      return;
    const needle = q.toLowerCase();
    const ranges: Range[] = [];
    const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT);
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      const text = node.nodeValue?.toLowerCase() ?? "";
      for (
        let at = text.indexOf(needle);
        at >= 0;
        at = text.indexOf(needle, at + needle.length)
      ) {
        const range = new Range();
        range.setStart(node, at);
        range.setEnd(node, at + needle.length);
        ranges.push(range);
      }
    }
    CSS.highlights.set("echo-search", new Highlight(...ranges));
    return () => void CSS.highlights.delete("echo-search");
  }, [element, q, pages]);
}
