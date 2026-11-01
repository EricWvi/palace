import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import type { ListedJotting } from "@/lib/api";
import {
  MarkdownContent,
  type ImagePolicy,
} from "@/components/markdown-content";
import { JottingTime, PendingDot } from "./jotting-card";

const media = /^\/api\/media\/[0-9a-f-]{36}$/;
// Only the owner's own uploads load; any other address, another site's included, stays a
// placeholder, so opening an old jotting never reaches outside. Jottings do not say what type a
// file is, so everything loads as an image.
const ownMedia: ImagePolicy = {
  kind: "load",
  resolve: (src) => (media.test(src) ? { kind: "image", src } : null),
};
// Each animation holds its last frame: the card and its backdrop are removed only after the
// closing animation ends, and falling back to their resting styles first would flash the card
// at full size in the middle of the screen for one frame.
const MOTION: KeyframeAnimationOptions = {
  duration: 300,
  easing: "ease-out",
  fill: "forwards",
};

// The card being read in full, and whether it is on its way back.
export type Zoom = { id: string; from: DOMRect; closing: boolean };

// What waits where the zoomed card shrinks back to. A recall card leaves the pile, so the zoomed
// card arrives at an empty slot and fades there. A wall card stays, and its folded face differs
// from the full text, most at the bottom, where a folded body fades and its footer sits. So
// the words fade first and a blank sheet flies back, the same paper and border as the card it
// lands on; there it fades and the card's own face shows through. Fading the whole card on the
// way loses its pale paper and border long before its dark words, which then fly on alone.
export type Landing = "empty" | "card";

// One jotting read in full, shared by the recall pile and the wall: it grows from its card to
// the middle of the screen, and on closing shrinks back to where the card is now and fades out,
// as `landing` says. What closing means for the card is the caller's, and so are the `actions`
// written down its right side. Escape and a click on the backdrop ask to close.
// See specs/decisions/web/echo/20261101-zoom-wall-jottings-by-their-time-and-load-palace-media.md
// and specs/decisions/web/echo/20261101-recall-least-completed-first-from-a-stored-daily-pile.md.
export function ZoomedJotting({
  jotting,
  zoom,
  landing,
  actions,
  slot,
  onClose,
  onClosed,
}: {
  jotting: ListedJotting;
  zoom: Zoom;
  landing: Landing;
  actions: ReactNode;
  slot: () => DOMRect;
  onClose: () => void;
  onClosed: () => void;
}) {
  // The frame carries the geometry; the card inside it scrolls, and the actions hang outside
  // its right edge, where the scrolling card would clip them.
  const frame = useRef<HTMLDivElement>(null);
  const card = useRef<HTMLElement>(null);
  const page = useRef<HTMLDivElement>(null);
  const backdrop = useRef<HTMLDivElement>(null);
  const [settled, setSettled] = useState(false);
  const opened = useRef(false);
  const closing = useRef(false);

  useLayoutEffect(() => {
    const element = frame.current;
    // Measured once: a second run would measure the card mid-growth.
    if (!element || !card.current || opened.current) return;
    opened.current = true;
    const width = Math.min(560, window.innerWidth - 32);
    element.style.width = `${width}px`;
    const height = Math.min(
      card.current.offsetHeight,
      window.innerHeight * 0.8,
    );
    const to = {
      left: (window.innerWidth - width) / 2,
      top: (window.innerHeight - height) / 2,
      width,
      height,
    };
    Object.assign(element.style, px(to));
    animate(backdrop.current, [{ opacity: 0 }, { opacity: 1 }]);
    void animate(element, [px(zoom.from), px(to)]).then(() => {
      // Closed before it finished growing: the shrinking now owns the card.
      if (closing.current) return;
      // Images arrive after the card was measured: once grown, the card follows its content,
      // centred so it grows evenly both ways. The held frame would pin the measured box.
      for (const growing of element.getAnimations?.() ?? []) growing.cancel();
      Object.assign(element.style, {
        top: "50%",
        height: "auto",
        transform: "translateY(-50%)",
      });
      // The cap comes with the class; waiting for React to add it would paint the card at its
      // full length for a frame first.
      element.classList.add("settled");
      setSettled(true);
    });
    // Measured once, on opening; closing is driven by `zoom.closing` below.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    const element = frame.current;
    if (!zoom.closing || !element || !card.current) return;
    closing.current = true;
    // Shrinks from the size the card has now, which images may have changed since it opened.
    const from = element.getBoundingClientRect();
    // A card scrolled down would shrink showing its middle; the card it lands on shows the top.
    card.current.scrollTop = 0;
    Object.assign(element.style, { ...px(from), transform: "none" });
    setSettled(false);
    const to = px(slot());
    animate(backdrop.current, [{ opacity: 1 }, { opacity: 0 }]);
    let frames: Keyframe[];
    if (landing === "empty")
      frames = [
        { ...px(from), opacity: 1 },
        { ...to, opacity: 1, offset: 0.6 },
        { ...to, opacity: 0 },
      ];
    else {
      animate(page.current, [
        { opacity: 1 },
        { opacity: 0, offset: 0.6 },
        { opacity: 0 },
      ]);
      // The card it lands on casts no shadow; the sheet loses its own as it lands.
      const shadow = getComputedStyle(card.current).boxShadow;
      animate(card.current, [
        { boxShadow: shadow },
        { boxShadow: "none", offset: 0.7 },
        { boxShadow: "none" },
      ]);
      frames = [
        { ...px(from), opacity: 1 },
        { ...to, opacity: 1, offset: 0.7 },
        { ...to, opacity: 0 },
      ];
    }
    void animate(element, frames).then(onClosed);
    // Closing runs once, when it is asked for.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [zoom.closing]);

  // Escape closes the card before the page or the pile sees it.
  useEffect(() => {
    if (zoom.closing) return;
    function escape(event: KeyboardEvent) {
      if (event.key !== "Escape") return;
      if ((event.target as Element).closest?.("input, textarea")) return;
      event.stopPropagation();
      onClose();
    }
    window.addEventListener("keydown", escape, { capture: true });
    return () =>
      window.removeEventListener("keydown", escape, { capture: true });
  }, [zoom.closing, onClose]);

  return (
    <>
      <div ref={backdrop} className="zoom-backdrop" onClick={onClose} />
      <div
        ref={frame}
        className={settled ? "zoom-frame settled" : "zoom-frame"}
      >
        <article
          ref={card}
          className="echo-card zoom"
          role="dialog"
          aria-label={jotting.title ?? "札记"}
        >
          {/* The words, apart from the sheet, so they can fade before it on the way back. */}
          <div ref={page}>
            {jotting.pending && <PendingDot />}
            {jotting.title && <h3>{jotting.title}</h3>}
            <div className="jot-body">
              <MarkdownContent content={jotting.body} images={ownMedia} />
            </div>
            <footer className="jot-foot">
              <JottingTime createdAt={jotting.created_at} form="full" />
              {jotting.tags.map((tag) => (
                <span key={tag}>#{tag}</span>
              ))}
            </footer>
          </div>
        </article>
        {/* Shown once the card has grown, gone the moment it starts back: they never fly. */}
        <div className="zoom-actions">{actions}</div>
      </div>
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
