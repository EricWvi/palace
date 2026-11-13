import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
} from "react";
import { createPortal } from "react-dom";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  completeRecall,
  dropJotting,
  jottingRecallOptions,
  patchRecall,
  type RecallNext,
} from "@/lib/api";
import {
  MarkdownContent,
  type ImagePolicy,
} from "@/components/markdown-content";
import { JottingTime, PendingDot } from "./jotting-card";
import { ZoomedJotting, type Zoom } from "./zoomed-jotting";

// A card in the pile is too small for pictures; they show when it is read in full.
const images: ImagePolicy = { kind: "placeholder" };
// Spread, the cards lie in rows of up to six, at most three rows: the server draws at most 18.
const MAX_COLUMNS = 6;
// Gap between spread cards; matches `--gap` in echo.css.
const GAP = 12;
// How far, in px, the bottom card of a pile lies from the top one. Up to five cards each sit
// 4px under the one above; a bigger pile packs its cards closer, so every card still shows an
// edge and the pile grows thicker rather than wider.
const DEPTH = 16;

// 今日回响: the day's recall cards dropped in a loose pile. A click spreads them over the wall in
// rows until a click anywhere else; a click on a spread card opens it in full. Only 温故 or
// 束阁 there takes a card out of the pile, and the server keeps that: closing the card any
// other way puts it back. `away` puts the pile aside while the wall is filtered.
// See specs/decisions/web/echo/20261101-recall-least-completed-first-from-a-stored-daily-pile.md.
export function RecallPile({ away }: { away: boolean }) {
  const client = useQueryClient();
  const recall = useQuery(jottingRecallOptions());
  const [spread, setSpread] = useState(false);
  // Once every card has started out, a card that moves up to fill a gap moves at once.
  const [settled, setSettled] = useState(false);
  const [columns, setColumns] = useState(MAX_COLUMNS);
  const [zoom, setZoom] = useState<Zoom | null>(null);
  // The open card was completed or deleted: it closes into an empty slot and leaves the pile.
  const [leaving, setLeaving] = useState<false | "done" | "deleted">(false);
  if (away && spread) setSpread(false);
  if (!spread && settled) setSettled(false);
  const pile = useRef<HTMLElement>(null);
  const date = recall.data?.date ?? "";
  const done = useMutation({
    mutationFn: ({ id, next }: { id: string; next: RecallNext }) =>
      completeRecall(date, id, next),
    onSuccess: ({ recall: state }) => {
      if (!zoom) return;
      setLeaving("done");
      setZoom({ ...zoom, closing: true });
      // The pile's copy waits until the card has landed, so the card keeps its slot meanwhile.
      patchRecall(client, zoom.id, { recall: state });
    },
  });

  // Any click outside the cards puts the spread pile away. It is caught before it lands, so the
  // click that closes the pile never also hits a tag or a fold in the dimmed wall underneath.
  useEffect(() => {
    if (!spread || zoom) return;
    function collapse(event: MouseEvent) {
      if ((event.target as Element).closest?.(".rc")) return;
      event.stopPropagation();
      event.preventDefault();
      setSpread(false);
    }
    document.addEventListener("click", collapse, { capture: true });
    return () =>
      document.removeEventListener("click", collapse, { capture: true });
  }, [spread, zoom]);

  // Escape puts the spread pile away before the page sees it; an open card takes it first.
  useEffect(() => {
    if (!spread || zoom) return;
    function escape(event: KeyboardEvent) {
      if (event.key !== "Escape") return;
      if ((event.target as Element).closest?.("input, textarea")) return;
      event.stopPropagation();
      setSpread(false);
    }
    window.addEventListener("keydown", escape, { capture: true });
    return () =>
      window.removeEventListener("keydown", escape, { capture: true });
  }, [spread, zoom]);

  // The rows follow the room to the right of the pile while it is spread.
  useEffect(() => {
    if (!spread) return;
    const measure = () => setColumns(columnsBeside(pile.current));
    window.addEventListener("resize", measure);
    return () => window.removeEventListener("resize", measure);
  }, [spread]);

  const all = recall.data?.jottings ?? [];
  const left = all.filter((card) => !card.done);
  useRowChanges(pile, spread ? left.map((card) => card.id) : [], columns);
  if (all.length === 0) return null;
  if (left.length === 0)
    return <p className="recall-done">今天的回响都已看过</p>;

  function choose(card: HTMLElement, id: string) {
    // A phone lays the pile out as a row already, so a tap reads the card.
    if (!spread && !window.matchMedia?.("(max-width: 720px)").matches) {
      setColumns(columnsBeside(pile.current));
      setSpread(true);
      return;
    }
    done.reset();
    setLeaving(false);
    setZoom({ id, from: card.getBoundingClientRect(), closing: false });
  }

  const zoomed = zoom && all.find((card) => card.id === zoom.id);
  // Spread cards fan out from the pile in a wave: the nearer a cell, the sooner its card moves.
  const rows = Math.ceil(left.length / columns);
  // Every card is a real card in the pile, so a read one takes one edge off it. Past five, the
  // cards lie closer and straighter, so a thick pile stays a neat one.
  const step = left.length <= 5 ? 4 : DEPTH / (left.length - 1);
  const sway = Math.min(1, 5 / left.length);
  const far = Math.min(left.length, columns) - 1 + rows - 1;
  return (
    <>
      <section
        ref={pile}
        className={["recall", spread && "spread", settled && "settled"]
          .filter(Boolean)
          .join(" ")}
        aria-label="今日回响"
        style={{ "--n": left.length, "--far": far } as CSSProperties}
        onTransitionEnd={(event) => {
          if (spread && event.target !== event.currentTarget) setSettled(true);
        }}
      >
        {left.map((card, i) => {
          const column = i % columns;
          const row = Math.floor(i / columns);
          return (
            <article
              key={card.id}
              className="echo-card rc"
              data-id={card.id}
              data-row={row}
              role="button"
              tabIndex={0}
              aria-label={card.title ?? card.body.slice(0, 40)}
              style={
                {
                  "--i": i,
                  "--step": `${step}px`,
                  "--x": column,
                  "--y": row,
                  "--wave": column + row,
                  "--r": `${tilt(card.id, i, sway)}deg`,
                  // Hidden while read in full; back as the open card lands on it, unless it is
                  // leaving.
                  visibility:
                    zoom?.id === card.id && !(zoom.closing && !leaving)
                      ? "hidden"
                      : undefined,
                } as CSSProperties
              }
              onClick={(event) => choose(event.currentTarget, card.id)}
              onKeyDown={(event) => {
                if (event.key === "Enter" || event.key === " ") {
                  event.preventDefault();
                  choose(event.currentTarget, card.id);
                }
              }}
            >
              {card.pending && <PendingDot />}
              {card.title && <h3>{card.title}</h3>}
              <div className="jot-body">
                <MarkdownContent
                  content={card.body}
                  images={images}
                  dialect="jotting"
                />
              </div>
              <footer className="jot-foot">
                <JottingTime createdAt={card.created_at} form="compact" />
                {card.tags.map((tag) => (
                  <span key={tag}>#{tag}</span>
                ))}
              </footer>
            </article>
          );
        })}
      </section>
      {zoom &&
        zoomed &&
        createPortal(
          <ZoomedJotting
            jotting={zoomed}
            zoom={zoom}
            landing={leaving ? "empty" : "card"}
            actions={(own) => [
              ...(["keep", "retire"] as const).map((next) => (
                <button
                  key={next}
                  type="button"
                  disabled={done.isPending || zoom.closing}
                  onClick={() => done.mutate({ id: zoom.id, next })}
                >
                  {next === "keep" ? "温故" : "束阁"}
                </button>
              )),
              own.edit,
              own.remove,
            ]}
            slot={() =>
              pile.current
                ?.querySelector(`[data-id="${zoom.id}"]`)
                ?.getBoundingClientRect() ?? zoom.from
            }
            onClose={() => {
              if (!done.isPending) setZoom({ ...zoom, closing: true });
            }}
            onClosed={() => {
              setZoom(null);
              if (leaving === "deleted") dropJotting(client, zoom.id);
              else if (leaving)
                patchRecall(client, zoom.id, {
                  recall: zoomed.recall,
                  done: true,
                });
            }}
            onDeleted={() => {
              setLeaving("deleted");
              setZoom({ ...zoom, closing: true });
            }}
          />,
          document.body,
        )}
    </>
  );
}

// How many spread cards fit side by side from the pile to the right edge of the page, up to six.
function columnsBeside(pile: HTMLElement | null) {
  const page = pile?.closest(".echo");
  const card = pile?.querySelector<HTMLElement>(".rc");
  if (!pile || !page || !card) return MAX_COLUMNS;
  const room =
    page.getBoundingClientRect().right - pile.getBoundingClientRect().left;
  const step = card.offsetWidth + GAP;
  if (step <= GAP) return MAX_COLUMNS;
  return Math.max(1, Math.min(MAX_COLUMNS, Math.floor((room + GAP) / step)));
}

// When a card leaves the spread pile, the cards after it move up a cell. One that moves to the
// end of the row above would cross the whole row on a slant; it fades out where it was and in
// where it goes instead, the way a word wraps to a line.
function useRowChanges(
  pile: { current: HTMLElement | null },
  ids: string[],
  columns: number,
) {
  const rows = useRef(new Map<string, number>());
  const key = `${columns}:${ids.join(" ")}`;
  useLayoutEffect(() => {
    const before = rows.current;
    const after = new Map(ids.map((id, i) => [id, Math.floor(i / columns)]));
    rows.current = after;
    const still = window.matchMedia?.(
      "(prefers-reduced-motion: reduce)",
    ).matches;
    // Only a card leaving changes rows here; a first spread or a new column count starts afresh.
    if (still || before.size !== after.size + 1) return;
    for (const [id, row] of after) {
      const was = before.get(id);
      const card = pile.current?.querySelector<HTMLElement>(
        `[data-id="${id}"]`,
      );
      if (was === undefined || was === row || !card?.animate) continue;
      const index = ids.indexOf(id);
      // Its old cell was one further on: the end of this row's predecessor in the old layout.
      const from = cell(index + 1, columns, card);
      const to = cell(index, columns, card);
      card.style.transition = "none";
      void card
        .animate(
          [
            { transform: from, opacity: 1 },
            { transform: from, opacity: 0, offset: 0.5 },
            { transform: to, opacity: 0, offset: 0.5 },
            { transform: to, opacity: 1 },
          ],
          { duration: 300, easing: "ease-out" },
        )
        .finished.then(() => card.style.removeProperty("transition"));
    }
    // Runs when the cards or the columns change, not on every render's fresh array.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
}

// Where the spread card at `index` sits, as the transform the spread layout gives it.
function cell(index: number, columns: number, card: HTMLElement) {
  const x = (index % columns) * (card.offsetWidth + GAP);
  const y = Math.floor(index / columns) * (card.offsetHeight + GAP);
  return `translate(${x}px, ${y}px)`;
}

// The top card lies almost straight; the ones under it are dropped more loosely, less so in a
// big pile (`sway` below 1). The angle comes from the id, so a card keeps its tilt as the cards
// above it leave.
function tilt(id: string, index: number, sway: number) {
  let hash = 0;
  for (const char of id) hash = (hash * 31 + char.charCodeAt(0)) | 0;
  const unit = ((hash >>> 0) % 1000) / 1000;
  return ((unit * 2 - 1) * (index === 0 ? 0.6 : 2.2 * sway)).toFixed(2);
}
