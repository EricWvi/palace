import { useLayoutEffect, useRef, useState } from "react";
import type { ListedJotting } from "@/lib/api";
import { absoluteTime, relativeTime } from "@/lib/jotting-time";
import {
  MarkdownContent,
  type ImagePolicy,
} from "@/components/markdown-content";

// Pictures would grow the cards after the wall has placed them; they show when a card is read
// in full.
const images: ImagePolicy = { kind: "placeholder" };
// Past this many lines a body opened in place would stretch its column over several screens.
const LONG_LINES = 40;

// How much there is below a card's fold; only known once laid out.
type Length = "fits" | "folds" | "long";

// One jotting on the wall. Long bodies fold at eight lines and open in place; the time, and the
// fold of a very long body, read the jotting in full over the wall instead. There is no jotting
// page to go to. `hidden` keeps the card's place while it is read in full.
export function JottingCard({
  jotting,
  activeTag,
  hidden,
  onTag,
  onZoom,
}: {
  jotting: ListedJotting;
  activeTag: string;
  hidden: boolean;
  onTag: (tag: string) => void;
  onZoom: (card: HTMLElement) => void;
}) {
  const card = useRef<HTMLElement>(null);
  const body = useRef<HTMLDivElement>(null);
  const [length, setLength] = useState<Length>("fits");
  const [open, setOpen] = useState(false);
  useLayoutEffect(() => {
    const element = body.current;
    if (!element) return;
    element.classList.add("folded");
    const full = element.scrollHeight;
    const lines = full / parseFloat(getComputedStyle(element).lineHeight);
    // A body that fits needs neither the fold nor its fade.
    if (full <= element.clientHeight + 2) {
      element.classList.remove("folded");
      setLength("fits");
    } else setLength(lines > LONG_LINES ? "long" : "folds");
  }, [jotting.body]);

  function zoom() {
    if (card.current) onZoom(card.current);
  }

  // Animates between the folded and the full height, then hands the height back to CSS.
  function toggle() {
    const element = body.current;
    if (!element) return;
    if (length === "long") return zoom();
    const opening = !open;
    const start = element.clientHeight;
    element.classList.remove("folded");
    const full = element.scrollHeight;
    const folded = parseFloat(getComputedStyle(element).lineHeight) * 8;
    element.style.maxHeight = `${start}px`;
    element.classList.add("moving");
    void element.offsetHeight; // commit the start height before transitioning
    element.style.maxHeight = `${opening ? full : folded}px`;
    const settle = () => {
      element.classList.remove("moving");
      element.style.maxHeight = "";
      if (!opening) element.classList.add("folded");
    };
    if (window.matchMedia?.("(prefers-reduced-motion: reduce)").matches)
      settle();
    else element.addEventListener("transitionend", settle, { once: true });
    setOpen(opening);
  }

  return (
    <article
      ref={card}
      className="jot echo-card"
      data-id={jotting.id}
      style={hidden ? { visibility: "hidden" } : undefined}
    >
      {jotting.pending && <PendingDot />}
      {jotting.title && <h3>{jotting.title}</h3>}
      {/* The fold classes change only in `toggle`: React rewriting them mid-transition would
          cut the animation short. */}
      <div ref={body} className="jot-body folded">
        <MarkdownContent
          content={jotting.body}
          images={images}
          dialect="jotting"
        />
      </div>
      {length !== "fits" && (
        <button
          type="button"
          className="jot-fold"
          aria-expanded={length === "long" ? undefined : open}
          onClick={toggle}
        >
          {open ? "收起" : "展开"}
        </button>
      )}
      {/* The tags take a line of their own above the time: side by side, a short relative time
          read as one more tag and a click meant to open the card filtered the wall instead. */}
      <footer className="jot-foot">
        {jotting.tags.length > 0 && (
          <div className="jot-tags">
            {jotting.tags.map((tag) => (
              <button
                key={tag}
                type="button"
                className="tag"
                aria-current={tag === activeTag ? "true" : undefined}
                onClick={() => onTag(tag)}
              >
                #{tag}
              </button>
            ))}
          </div>
        )}
        <button
          type="button"
          className="when"
          aria-label="放大"
          aria-haspopup="dialog"
          onClick={zoom}
        >
          <JottingTime createdAt={jotting.created_at} />
        </button>
      </footer>
    </article>
  );
}

// 待整理: a passing state of the card, so a dot in the corner padding that names itself on hover.
export function PendingDot() {
  return <span className="pending" data-tip="待整理" aria-label="待整理" />;
}

// How a card writes its time: on the wall, relative inside seven days and full past that; on
// the narrow recall cards the same, but an old date without its clock time; read in full,
// always the full time. None has a tooltip: the full time is one click away.
type TimeForm = "wall" | "compact" | "full";

export function JottingTime({
  createdAt,
  form = "wall",
}: {
  createdAt: number;
  form?: TimeForm;
}) {
  // The wording is taken when the card appears; it is not a clock that ticks.
  const [now] = useState(() => Date.now());
  const absolute = absoluteTime(createdAt);
  const relative = form === "full" ? null : relativeTime(createdAt, now);
  return (
    <time className="time" dateTime={new Date(createdAt).toISOString()}>
      {relative ?? (form === "compact" ? absolute.slice(0, 10) : absolute)}
    </time>
  );
}
