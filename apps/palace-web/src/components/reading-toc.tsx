import { useCallback, useEffect, useRef, useState } from "react";

export type HeadingLevel = 1 | 2 | 3 | 4 | 5 | 6;

// Callers own the targets and labels; this module owns reading, jumping and hover intent.
export type TocItem = { id: string; label: string } & (
  { kind: "message"; role: string } | { kind: "heading"; level: HeadingLevel }
);

// The last target above the reading line, or the final one when no more scrolling is possible.
function readingIndex(items: TocItem[]) {
  const line = window.innerHeight / 3;
  let current = 0;
  items.forEach((item, index) => {
    const top = document.getElementById(item.id)?.getBoundingClientRect().top;
    if (top !== undefined && top <= line) current = index;
  });
  const bottom = document.documentElement.scrollHeight - 2;
  if (window.innerHeight + window.scrollY >= bottom) current = items.length - 1;
  return current;
}

// A shared reading rail: messages supply role marks, notes supply heading levels. Hover intent,
// keyboard access and pinned scroll targets stay identical for both kinds of reading page.
export function ReadingToc({
  items,
  label,
}: {
  items: TocItem[];
  label: string;
}) {
  const [current, setCurrent] = useState(0);
  const [hoverOpen, setHoverOpen] = useState(false);
  const hoverTimer = useRef<number | undefined>(undefined);
  // A jump names its target outright: a short message near the end may never reach the reading
  // line, and must not hand the mark to its neighbour. The pin holds until the reader scrolls.
  const pinned = useRef<number | null>(null);
  const toc = useRef<HTMLElement>(null);
  const panel = useRef<HTMLOListElement>(null);

  const cancelHover = useCallback(() => {
    window.clearTimeout(hoverTimer.current);
    hoverTimer.current = undefined;
    setHoverOpen(false);
  }, []);

  useEffect(() => {
    // Fast window exits can omit the element-level leave event; browser boundaries cancel too.
    const root = document.documentElement;
    const onMouseOut = (event: MouseEvent) => {
      if (event.relatedTarget === null) cancelHover();
    };
    const onVisibilityChange = () => {
      if (document.hidden) cancelHover();
    };
    root.addEventListener("mouseleave", cancelHover);
    root.addEventListener("pointerleave", cancelHover);
    window.addEventListener("mouseout", onMouseOut);
    window.addEventListener("blur", cancelHover);
    document.addEventListener("visibilitychange", onVisibilityChange);
    return () => {
      window.clearTimeout(hoverTimer.current);
      root.removeEventListener("mouseleave", cancelHover);
      root.removeEventListener("pointerleave", cancelHover);
      window.removeEventListener("mouseout", onMouseOut);
      window.removeEventListener("blur", cancelHover);
      document.removeEventListener("visibilitychange", onVisibilityChange);
    };
  }, [cancelHover]);

  useEffect(() => {
    pinned.current = null;
    let frame = 0;
    const mark = () => {
      frame = 0;
      setCurrent(pinned.current ?? readingIndex(items));
    };
    const onScroll = () => {
      if (!frame) frame = requestAnimationFrame(mark);
    };
    const unpin = () => {
      if (pinned.current === null) return;
      pinned.current = null;
      onScroll();
    };
    mark();
    window.addEventListener("scroll", onScroll, { passive: true });
    window.addEventListener("resize", onScroll);
    const reader = ["wheel", "touchmove", "keydown"] as const;
    for (const type of reader)
      window.addEventListener(type, unpin, { passive: true });
    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener("scroll", onScroll);
      window.removeEventListener("resize", onScroll);
      for (const type of reader) window.removeEventListener(type, unpin);
    };
  }, [items]);

  // A list of one says nothing the page does not.
  if (items.length < 2) return null;

  // A long list opens on the reader's place, not on its first line.
  function centerCurrent() {
    const list = panel.current;
    const link = list?.querySelector<HTMLElement>('[aria-current="true"]');
    if (list && link)
      list.scrollTop =
        link.offsetTop - (list.clientHeight - link.offsetHeight) / 2;
  }

  return (
    <nav
      ref={toc}
      className="toc"
      aria-label={label}
      data-open={hoverOpen || undefined}
      onMouseEnter={() => {
        // Keep the invisible panel out of hit testing until the reader intends to open it.
        window.clearTimeout(hoverTimer.current);
        hoverTimer.current = window.setTimeout(() => {
          hoverTimer.current = undefined;
          if (!toc.current?.matches(":hover") || document.hidden) return;
          centerCurrent();
          setHoverOpen(true);
        }, 350);
      }}
      onMouseLeave={cancelHover}
    >
      <div className="toc-rail" aria-hidden="true">
        {items.map((item, index) => (
          <span
            key={item.id}
            className={item.kind === "message" ? item.role : undefined}
            data-level={item.kind === "heading" ? item.level : undefined}
            data-current={index === current || undefined}
          />
        ))}
      </div>
      <ol
        ref={panel}
        className="toc-panel"
        // A click must not leave focus on the line, or :focus-within keeps the list open after
        // the pointer leaves. Keyboard users still land on it with Enter and keep it open.
        onMouseDown={(event) => event.preventDefault()}
      >
        {items.map((item, index) => (
          <li key={item.id}>
            <a
              className={item.kind === "message" ? item.role : undefined}
              data-level={item.kind === "heading" ? item.level : undefined}
              href={`#${item.id}`}
              title={item.label}
              aria-current={index === current || undefined}
              onClick={(event) => {
                event.preventDefault();
                pinned.current = index;
                setCurrent(index);
                // Jumping is reading too: the address keeps the path and gains no history entry.
                document.getElementById(item.id)?.scrollIntoView({
                  behavior: window.matchMedia?.(
                    "(prefers-reduced-motion: reduce)",
                  ).matches
                    ? "auto"
                    : "smooth",
                });
              }}
            >
              {item.label}
            </a>
          </li>
        ))}
      </ol>
    </nav>
  );
}
