import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
} from "react";
import { createPortal } from "react-dom";
import { useQuery } from "@tanstack/react-query";
import { jottingRecallOptions, type ListedJotting } from "@/lib/api";
import {
  MarkdownContent,
  type ImagePolicy,
} from "@/components/markdown-content";
import { JottingTime, PendingDot } from "./jotting-card";

const images: ImagePolicy = { kind: "placeholder" };
const MOTION = { duration: 300, easing: "ease-out" };

// The card being read in full, and whether it is on its way back.
type Zoom = { id: string; from: DOMRect; closing: boolean };

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

  // Escape closes the open card, then the spread pile, before the page sees it.
  useEffect(() => {
    if (!spread && !zoom) return;
    function escape(event: KeyboardEvent) {
      if (event.key !== "Escape") return;
      if ((event.target as Element).closest?.("input, textarea")) return;
      event.stopPropagation();
      if (zoom) setZoom({ ...zoom, closing: true });
      else setSpread(false);
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
            <RecallContent jotting={jotting} compact />
          </article>
        ))}
      </section>
      {zoom &&
        zoomed &&
        createPortal(
          <ZoomedCard
            jotting={zoomed}
            zoom={zoom}
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

function RecallContent({
  jotting,
  compact,
}: {
  jotting: ListedJotting;
  compact?: boolean;
}) {
  return (
    <>
      {jotting.pending && <PendingDot />}
      {jotting.title && <h3>{jotting.title}</h3>}
      <div className="jot-body">
        <MarkdownContent content={jotting.body} images={images} />
      </div>
      <footer className="jot-foot">
        <JottingTime createdAt={jotting.created_at} compact={compact} />
        {jotting.tags.map((tag) => (
          <span key={tag}>#{tag}</span>
        ))}
      </footer>
    </>
  );
}

// One card read in full: it grows from its place in the pile to the middle of the screen, and
// on closing shrinks back to that place and fades there.
function ZoomedCard({
  jotting,
  zoom,
  slot,
  onClose,
  onClosed,
}: {
  jotting: ListedJotting;
  zoom: Zoom;
  slot: () => DOMRect;
  onClose: () => void;
  onClosed: () => void;
}) {
  const card = useRef<HTMLElement>(null);
  const backdrop = useRef<HTMLDivElement>(null);
  const target = useRef<Box | null>(null);
  const [settled, setSettled] = useState(false);

  useLayoutEffect(() => {
    const element = card.current;
    // Measured once: a second run would measure the card mid-growth.
    if (!element || target.current) return;
    const width = Math.min(560, window.innerWidth - 32);
    element.style.width = `${width}px`;
    const height = Math.min(element.offsetHeight, window.innerHeight * 0.8);
    const to = {
      left: (window.innerWidth - width) / 2,
      top: (window.innerHeight - height) / 2,
      width,
      height,
    };
    target.current = to;
    Object.assign(element.style, px(to));
    const growing = animate(element, [px(zoom.from), px(to)]);
    animate(backdrop.current, [{ opacity: 0 }, { opacity: 1 }]);
    void growing.then(() => setSettled(true));
    // Measured once, on opening; closing is driven by `zoom.closing` below.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (!zoom.closing || !card.current || !target.current) return;
    setSettled(false);
    const to = px(slot());
    animate(backdrop.current, [{ opacity: 1 }, { opacity: 0 }]);
    void animate(card.current, [
      { ...px(target.current), opacity: 1 },
      { ...to, opacity: 1, offset: 0.6 },
      { ...to, opacity: 0 },
    ]).then(onClosed);
    // Closing runs once, when it is asked for.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [zoom.closing]);

  return (
    <>
      <div ref={backdrop} className="zoom-backdrop" onClick={onClose} />
      <article
        ref={card}
        className={settled ? "echo-card zoom settled" : "echo-card zoom"}
        role="dialog"
        aria-label={jotting.title ?? "回响"}
      >
        <RecallContent jotting={jotting} />
      </article>
    </>
  );
}

type Box = { left: number; top: number; width: number; height: number };

function px(box: Box) {
  return {
    left: `${box.left}px`,
    top: `${box.top}px`,
    width: `${box.width}px`,
    height: `${box.height}px`,
  };
}

// Runs a Web Animation and resolves when it ends; at once where motion is reduced or the
// platform cannot animate.
function animate(element: Element | null, frames: Keyframe[]): Promise<void> {
  const still = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
  if (!element || still || typeof element.animate !== "function")
    return Promise.resolve();
  return element.animate(frames, MOTION).finished.then(() => undefined);
}

// The top card lies almost straight; the ones under it are dropped more loosely. The angle comes
// from the id, so a card keeps its tilt as the cards above it leave.
function tilt(id: string, index: number) {
  let hash = 0;
  for (const char of id) hash = (hash * 31 + char.charCodeAt(0)) | 0;
  const unit = ((hash >>> 0) % 1000) / 1000;
  return ((unit * 2 - 1) * (index === 0 ? 0.6 : 2.2)).toFixed(2);
}
