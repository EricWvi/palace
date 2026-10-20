import { useLayoutEffect, useRef, useState } from "react";
import type { ListedJotting } from "@/lib/api";
import { absoluteTime, relativeTime } from "@/lib/jotting-time";
import {
  MarkdownContent,
  type ImagePolicy,
} from "@/components/markdown-content";

// Attachments are not served yet, so a jotting's images stay placeholders.
const images: ImagePolicy = { kind: "placeholder" };

// One jotting on the wall. Long bodies fold at eight lines and open in place; there is no
// jotting page to go to.
export function JottingCard({
  jotting,
  activeTag,
  onTag,
}: {
  jotting: ListedJotting;
  activeTag: string;
  onTag: (tag: string) => void;
}) {
  const body = useRef<HTMLDivElement>(null);
  // Whether the body overflows its fold; only known once laid out.
  const [foldable, setFoldable] = useState(false);
  const [open, setOpen] = useState(false);
  useLayoutEffect(() => {
    const element = body.current;
    if (!element) return;
    element.classList.add("folded");
    const overflowing = element.scrollHeight > element.clientHeight + 2;
    // A body that fits needs neither the fold nor its fade.
    if (!overflowing) element.classList.remove("folded");
    setFoldable(overflowing);
  }, [jotting.body]);

  // Animates between the folded and the full height, then hands the height back to CSS.
  function toggle() {
    const element = body.current;
    if (!element) return;
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
    <article className="jot echo-card" data-id={jotting.id}>
      {jotting.pending && <PendingDot />}
      {jotting.title && <h3>{jotting.title}</h3>}
      {/* The fold classes change only in `toggle`: React rewriting them mid-transition would
          cut the animation short. */}
      <div ref={body} className="jot-body folded">
        <MarkdownContent content={jotting.body} images={images} />
      </div>
      {foldable && (
        <button
          type="button"
          className="jot-fold"
          aria-expanded={open}
          onClick={toggle}
        >
          {open ? "收起" : "展开"}
        </button>
      )}
      <footer className="jot-foot">
        <JottingTime createdAt={jotting.created_at} />
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
      </footer>
    </article>
  );
}

// 待整理: a passing state of the card, so a dot in the corner padding that names itself on hover.
export function PendingDot() {
  return <span className="pending" data-tip="待整理" aria-label="待整理" />;
}

// A relative time inside seven days with the full time on hover; past that, the full time. The
// compact form, for the narrow recall cards, shows an old date without its clock time and gives
// a recent one no tooltip, which the clipped card would cut.
export function JottingTime({
  createdAt,
  compact,
}: {
  createdAt: number;
  compact?: boolean;
}) {
  // The wording is taken when the card appears; it is not a clock that ticks.
  const [now] = useState(() => Date.now());
  const relative = relativeTime(createdAt, now);
  const absolute = absoluteTime(createdAt);
  return (
    <time
      className="time"
      dateTime={new Date(createdAt).toISOString()}
      data-tip={relative && !compact ? absolute : undefined}
    >
      {relative ?? (compact ? absolute.slice(0, 10) : absolute)}
    </time>
  );
}
