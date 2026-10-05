import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { Link } from "react-router-dom";
import {
  sources,
  type Moment,
  type MomentKind,
  type MomentOutline,
} from "@/lib/api";

const MORPH = { duration: 280, easing: "cubic-bezier(0.2, 0, 0, 1)" };

// Each kind draws exactly one placeholder; a new kind does not type-check until it has one.
const skeletons: Record<MomentKind, React.ReactNode> = {
  conversation: (
    <>
      <span className="bar head" />
      <span className="bar meta" />
      <span className="excerpt">
        <span className="bar" />
        <span className="bar short" />
      </span>
    </>
  ),
};

// What one list item shows: a placeholder still waiting, a card, or a placeholder whose moment
// the cards no longer contain and which is collapsing away.
type Row =
  | { kind: "skeleton"; outline: MomentOutline }
  | { kind: "card"; moment: Moment; from: "skeleton" | "nothing" }
  | { kind: "leaving"; outline: MomentOutline };

// Motion is a nicety: without the Web Animations API, or when the reader asked for less motion,
// cards simply replace their placeholders.
function animates() {
  return (
    typeof Element.prototype.animate === "function" &&
    !window.matchMedia?.("(prefers-reduced-motion: reduce)").matches
  );
}

// Lays out the cards of the next rows; placeholders the cards no longer contain stay where they
// were so they can collapse in place instead of jumping to the end.
function morphRows(moments: Moment[], placeholders: MomentOutline[]): Row[] {
  const arriving = new Set(placeholders.map((outline) => outline.id));
  const rows: Row[] = moments.map((moment) => ({
    kind: "card",
    moment,
    from: arriving.has(moment.id) ? "skeleton" : "nothing",
  }));
  const kept = new Set(moments.map((moment) => moment.id));
  placeholders.forEach((outline, index) => {
    if (!kept.has(outline.id))
      rows.splice(Math.min(index, rows.length), 0, {
        kind: "leaving",
        outline,
      });
  });
  return rows;
}

/**
 * One day's moments. Before the cards arrive it draws a placeholder per outline entry; when they
 * arrive each placeholder grows into the card with its own id, in the same list item, instead of
 * the whole list being swapped at once. `onSettled` runs once the cards are in their final place.
 */
export function DayMoments({
  outline,
  moments,
  date,
  onSettled,
}: {
  outline: MomentOutline[] | undefined;
  moments: Moment[] | undefined;
  date: string;
  onSettled: () => void;
}) {
  const list = useRef<HTMLOListElement>(null);
  // Where each placeholder sat in the last layout, so its card can start from there.
  const placed = useRef(new Map<string, { top: number; height: number }>());
  // The placeholders on screen while the cards are still loading.
  const [waiting, setWaiting] = useState<MomentOutline[] | null>(null);
  // The placeholders being turned into cards; null once the cards have settled.
  const [morphing, setMorphing] = useState<MomentOutline[] | null>(null);

  // Decide during render, so the frame that first shows the cards already knows they morph.
  if (moments === undefined && outline && outline !== waiting)
    setWaiting(outline);
  if (moments !== undefined && waiting) {
    setWaiting(null);
    if (animates()) setMorphing(waiting);
  }

  const rows: Row[] =
    moments === undefined
      ? (outline ?? []).map((entry) => ({ kind: "skeleton", outline: entry }))
      : morphing
        ? morphRows(moments, morphing)
        : moments.map((moment) => ({ kind: "card", moment, from: "nothing" }));

  // Remember where the placeholders are after every layout while they wait; a morph reads it.
  useLayoutEffect(() => {
    if (morphing) return;
    const items = [...(list.current?.children ?? [])] as HTMLElement[];
    placed.current = new Map(
      items
        .filter((item) => item.dataset.state === "skeleton")
        .map((item) => [
          item.dataset.id!,
          { top: item.offsetTop, height: item.offsetHeight },
        ]),
    );
  });

  useLayoutEffect(() => {
    if (!morphing) return;
    const items = [...(list.current?.children ?? [])] as HTMLElement[];
    // FLIP: every item starts where and as tall as its placeholder was, then eases to its own
    // layout. Leaving placeholders collapse; cards nothing announced grow from nothing.
    const animations = items.map((item) => {
      const was = placed.current.get(item.dataset.id!);
      const now = { top: item.offsetTop, height: item.offsetHeight };
      if (item.dataset.state === "leaving")
        return item.animate(
          [
            { height: `${now.height}px`, opacity: 1 },
            { height: "0px", opacity: 0 },
          ],
          { ...MORPH, fill: "forwards" },
        );
      if (!was)
        return item.animate(
          [
            { height: "0px", opacity: 0 },
            { height: `${now.height}px`, opacity: 1 },
          ],
          MORPH,
        );
      return item.animate(
        [
          {
            height: `${was.height}px`,
            transform: `translateY(${was.top - now.top}px)`,
          },
          { height: `${now.height}px`, transform: "none" },
        ],
        MORPH,
      );
    });
    let live = true;
    Promise.all(animations.map((animation) => animation.finished))
      .then(() => {
        if (live) setMorphing(null);
      })
      // Cancelled by a newer morph or by leaving the day; whoever cancelled takes over.
      .catch(() => {});
    return () => {
      live = false;
      for (const animation of animations) animation.cancel();
    };
  }, [morphing]);

  const settled = moments !== undefined && !morphing;
  useEffect(() => {
    if (settled) onSettled();
  }, [settled, onSettled]);

  if (rows.length === 0)
    return moments !== undefined || outline !== undefined ? (
      <p className="quiet-note">这一天还没有记录。</p>
    ) : null;
  return (
    <ol ref={list} className="timeline" aria-busy={moments === undefined}>
      {rows.map((row) => (
        <MomentItem
          key={row.kind === "card" ? row.moment.id : row.outline.id}
          row={row}
          date={date}
          morphing={morphing !== null}
        />
      ))}
    </ol>
  );
}

// One list item through all its states. It is always this component's <li>, so a placeholder
// and the card it becomes are the same element and React never remounts it mid-morph.
function MomentItem({
  row,
  date,
  morphing,
}: {
  row: Row;
  date: string;
  morphing: boolean;
}) {
  switch (row.kind) {
    case "skeleton":
    case "leaving":
      return (
        <li
          className="moment"
          data-id={row.outline.id}
          data-state={row.kind}
          aria-hidden="true"
        >
          <span className="skeleton">{skeletons[row.outline.kind]}</span>
        </li>
      );
    case "card": {
      // A conversation's full text belongs to 摘星, so its card only says what happened and
      // links there. While arriving, the placeholder it grows out of fades away on top of it.
      const { moment } = row;
      const arriving = morphing && row.from === "skeleton";
      const source = sources[moment.source];
      return (
        <li
          className="moment"
          id={`moment-${moment.id}`}
          data-id={moment.id}
          data-state={
            !morphing
              ? "card"
              : row.from === "skeleton"
                ? "arriving"
                : "entering"
          }
        >
          <Link
            className="moment-head"
            to={`/conversations/${moment.conversation_id}?path=${moment.id}&date=${date}`}
          >
            <span className="kind">对话</span>
            <span className="title">{moment.title}</span>
          </Link>
          <p className="meta">
            <span>{source}</span>
            <span>{moment.message_count} 条消息</span>
          </p>
          {moment.excerpt.length > 0 && (
            <blockquote className="chat">
              {moment.excerpt.map((line, index) => (
                <p key={index}>
                  {line.role === "user" ? "你" : source}：{line.text}
                </p>
              ))}
            </blockquote>
          )}
          {arriving && (
            <span className="skeleton fading" aria-hidden="true">
              {skeletons[moment.kind]}
            </span>
          )}
        </li>
      );
    }
  }
}
