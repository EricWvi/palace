import { useEffect, useRef, useState, type CSSProperties } from "react";
import { createPortal } from "react-dom";
import { useQuery } from "@tanstack/react-query";
import { jottingRecallOptions } from "@/lib/api";
import {
  MarkdownContent,
  type ImagePolicy,
} from "@/components/markdown-content";
import { JottingTime, PendingDot } from "./jotting-card";
import { ZoomedJotting, type Zoom } from "./zoomed-jotting";

// A card in the pile is too small for pictures; they show when it is read in full.
const images: ImagePolicy = { kind: "placeholder" };

// 今日回响: the day's recall cards dropped in a loose pile. A click spreads them over the wall
// until a click anywhere else; a click on a spread card opens it in place, and closing it counts
// as read, so it leaves the pile. Which cards were read lives only here: leaving the page or
// reloading brings the whole pile back, while the server keeps the pile itself fixed for the day.
// `away` puts the pile aside while the wall is filtered.
export function RecallPile({ away }: { away: boolean }) {
  const recall = useQuery(jottingRecallOptions());
  const [read, setRead] = useState<ReadonlySet<string>>(new Set());
  const [spread, setSpread] = useState(false);
  const [zoom, setZoom] = useState<Zoom | null>(null);
  if (away && spread) setSpread(false);
  const pile = useRef<HTMLElement>(null);

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

  const all = recall.data?.jottings ?? [];
  if (all.length === 0) return null;
  const left = all.filter((jotting) => !read.has(jotting.id));
  if (left.length === 0)
    return <p className="recall-done">今天的回响都已看过</p>;

  function choose(card: HTMLElement, id: string) {
    // A phone lays the pile out as a row already, so a tap reads the card.
    if (!spread && !window.matchMedia?.("(max-width: 720px)").matches) {
      setSpread(true);
      return;
    }
    setZoom({ id, from: card.getBoundingClientRect(), closing: false });
  }

  const zoomed = zoom && all.find((jotting) => jotting.id === zoom.id);
  return (
    <>
      <section
        ref={pile}
        className={spread ? "recall spread" : "recall"}
        aria-label="今日回响"
        style={{ "--n": left.length } as CSSProperties}
      >
        {left.map((jotting, i) => (
          <article
            key={jotting.id}
            className="echo-card rc"
            data-id={jotting.id}
            role="button"
            tabIndex={0}
            aria-label={jotting.title ?? jotting.body.slice(0, 40)}
            style={
              {
                "--i": i,
                "--r": `${tilt(jotting.id, i)}deg`,
                visibility: zoom?.id === jotting.id ? "hidden" : undefined,
              } as CSSProperties
            }
            onClick={(event) => choose(event.currentTarget, jotting.id)}
            onKeyDown={(event) => {
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                choose(event.currentTarget, jotting.id);
              }
            }}
          >
            {jotting.pending && <PendingDot />}
            {jotting.title && <h3>{jotting.title}</h3>}
            <div className="jot-body">
              <MarkdownContent content={jotting.body} images={images} />
            </div>
            <footer className="jot-foot">
              <JottingTime createdAt={jotting.created_at} form="compact" />
              {jotting.tags.map((tag) => (
                <span key={tag}>#{tag}</span>
              ))}
            </footer>
          </article>
        ))}
      </section>
      {zoom &&
        zoomed &&
        createPortal(
          <ZoomedJotting
            jotting={zoomed}
            zoom={zoom}
            landing="empty"
            slot={() =>
              pile.current
                ?.querySelector(`[data-id="${zoom.id}"]`)
                ?.getBoundingClientRect() ?? zoom.from
            }
            onClose={() => setZoom({ ...zoom, closing: true })}
            onClosed={() => {
              setZoom(null);
              setRead((before) => new Set(before).add(zoom.id));
            }}
          />,
          document.body,
        )}
    </>
  );
}

// The top card lies almost straight; the ones under it are dropped more loosely. The angle comes
// from the id, so a card keeps its tilt as the cards above it leave.
function tilt(id: string, index: number) {
  let hash = 0;
  for (const char of id) hash = (hash * 31 + char.charCodeAt(0)) | 0;
  const unit = ((hash >>> 0) % 1000) / 1000;
  return ((unit * 2 - 1) * (index === 0 ? 0.6 : 2.2)).toFixed(2);
}
