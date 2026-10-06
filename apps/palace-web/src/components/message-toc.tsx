import { useEffect, useRef, useState } from "react";
import type { Message } from "@/lib/api";

// The element id a message of the reading page carries, so the table of contents can jump to it.
export function messageAnchor(message: Message) {
  return `message-${message.id}`;
}

// Which message the reader is on: the last one whose top has passed the upper third of the
// window, or the last message once the page cannot scroll any further.
function readingIndex(messages: Message[]) {
  const line = window.innerHeight / 3;
  let current = 0;
  messages.forEach((message, index) => {
    const top = document
      .getElementById(messageAnchor(message))
      ?.getBoundingClientRect().top;
    if (top !== undefined && top <= line) current = index;
  });
  const bottom = document.documentElement.scrollHeight - 2;
  if (window.innerHeight + window.scrollY >= bottom)
    current = messages.length - 1;
  return current;
}

// A tick per message of the path being read, pinned to the right edge; pointing at the ticks or
// tabbing into them opens one line per message, each its opening words from the server. Only
// shown where the margin beside the sheet has room for it.
export function MessageToc({ messages }: { messages: Message[] }) {
  const [current, setCurrent] = useState(0);
  // A jump names its target outright: a short message near the end may never reach the reading
  // line, and must not hand the mark to its neighbour. The pin holds until the reader scrolls.
  const pinned = useRef<number | null>(null);
  const panel = useRef<HTMLOListElement>(null);

  useEffect(() => {
    pinned.current = null;
    let frame = 0;
    const mark = () => {
      frame = 0;
      setCurrent(pinned.current ?? readingIndex(messages));
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
  }, [messages]);

  // A list of one says nothing the page does not.
  if (messages.length < 2) return null;

  // A long list opens on the reader's place, not on its first line.
  function centerCurrent() {
    const list = panel.current;
    const link = list?.querySelector<HTMLElement>('[aria-current="true"]');
    if (list && link)
      list.scrollTop =
        link.offsetTop - (list.clientHeight - link.offsetHeight) / 2;
  }

  return (
    <nav className="toc" aria-label="消息目录" onMouseEnter={centerCurrent}>
      <div className="toc-rail" aria-hidden="true">
        {messages.map((message, index) => (
          <span
            key={message.id}
            className={message.role}
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
        {messages.map((message, index) => (
          <li key={message.id}>
            <a
              className={message.role}
              href={`#${messageAnchor(message)}`}
              title={message.toc_line}
              aria-current={index === current || undefined}
              onClick={(event) => {
                event.preventDefault();
                pinned.current = index;
                setCurrent(index);
                // Jumping is reading too: the address keeps the path and gains no history entry.
                document
                  .getElementById(messageAnchor(message))
                  ?.scrollIntoView({
                    behavior: window.matchMedia?.(
                      "(prefers-reduced-motion: reduce)",
                    ).matches
                      ? "auto"
                      : "smooth",
                  });
              }}
            >
              {message.toc_line || "（空消息）"}
            </a>
          </li>
        ))}
      </ol>
    </nav>
  );
}
