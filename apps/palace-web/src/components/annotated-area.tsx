import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
import type { AnnotationStyle } from "@/lib/annotation-paint";
import { selectedText, type SelectedText } from "@/lib/annotation-dom";
import { AnnotationBar, styleKeys } from "./annotation-bar";
import { NoteCard } from "./note-card";
import { draftId, type Anchor, type Annotations } from "./annotation-layer";

// The margin column needs this much window; below it notes float under their marks instead.
const marginQuery = "(min-width: 1180px)";
// A card sits level with its mark's first line; its padding is about this much above the text.
const cardInset = 9;
const cardGap = 10;

// Whether the window is wide enough for the margin column, following resizes.
function useWide() {
  const [wide, setWide] = useState(
    () => window.matchMedia?.(marginQuery).matches ?? false,
  );
  useEffect(() => {
    const query = window.matchMedia?.(marginQuery);
    if (!query) return;
    const change = () => setWide(query.matches);
    query.addEventListener("change", change);
    return () => query.removeEventListener("change", change);
  }, []);
  return wide;
}

type Bar =
  | { kind: "selection"; anchor: Anchor; at: { x: number; y: number } }
  | { kind: "annotation"; id: string; at: { x: number; y: number } };

type Box = Pick<DOMRect, "top" | "bottom" | "left" | "width">;
// Where a selection is when the browser cannot say (no layout, as in tests): the window's corner.
const nowhere: Box = { top: 0, bottom: 0, left: 0, width: 0 };

// Page coordinates for a bar centred above `rect`, or below it when the top of the window is near.
function barPoint(rect: Box) {
  const above = rect.top > 56;
  return {
    x: rect.left + rect.width / 2 + window.scrollX,
    y: (above ? rect.top - 8 : rect.bottom + 48) + window.scrollY,
  };
}

const marksOf = (root: ParentNode, id: string) => [
  ...root.querySelectorAll<HTMLElement>(`mark[data-annotation="${id}"]`),
];

const typing = (target: EventTarget | null) =>
  target instanceof Element &&
  target.closest("input, textarea, select, [contenteditable='true']") !== null;

// The text column of a reading page with its annotations: selecting text offers the bar,
// clicking a mark offers it for that mark, and notes sit in the margin when the window allows,
// otherwise in a card under the mark. Marks and notes light each other up on hover.
export function AnnotatedArea({
  layer,
  children,
}: {
  layer: Annotations;
  children: ReactNode;
}) {
  const area = useRef<HTMLDivElement>(null);
  const wide = useWide();
  const margin = wide && layer.hasNotes;
  const [bar, setBar] = useState<Bar | null>(null);
  const [hot, setHot] = useState<string | null>(null);
  // The card being worked with: pinned level with its mark in the margin.
  const [active, setActive] = useState<string | null>(null);
  const [editing, setEditing] = useState<string | null>(null);
  // Narrow windows: the note floating under its mark, and where it floats.
  const [floating, setFloating] = useState<string | null>(null);
  const [floatAt, setFloatAt] = useState<{ x: number; y: number } | null>(null);
  const [tops, setTops] = useState<Record<string, number>>({});
  const [layoutTick, setLayoutTick] = useState(0);
  const cards = useRef(new Map<string, HTMLDivElement>());
  const hoverTimer = useRef<ReturnType<typeof setTimeout>>(undefined);

  const find = useCallback(
    (id: string) => layer.list.find((item) => item.id === id),
    [layer.list],
  );

  // Notes to show: every saved one, one being added to an annotation, and a draft.
  const notes = useMemo(
    () => [
      ...layer.list
        .filter((item) => item.note !== null || editing === item.id)
        .map((item) => ({
          id: item.id,
          style: item.style,
          note: item.note ?? "",
        })),
      ...(layer.draft ? [{ id: draftId, style: null, note: "" }] : []),
    ],
    [layer.list, layer.draft, editing],
  );

  // Lights a note and every run of its mark together, without re-rendering the text.
  useEffect(() => {
    const root = area.current;
    if (!root) return;
    for (const mark of root.querySelectorAll("mark.anno.hot"))
      mark.classList.remove("hot");
    if (hot) for (const mark of marksOf(root, hot)) mark.classList.add("hot");
  });

  // Margin cards sit level with their marks and push each other down; the card being worked
  // with is pinned level with its mark, and the ones above it give way upward.
  useLayoutEffect(() => {
    const root = area.current;
    if (!margin || !root) return;
    const origin = root.getBoundingClientRect().top;
    const placed = notes
      .map((note) => {
        const mark = marksOf(root, note.id)[0];
        const card = cards.current.get(note.id);
        if (!mark || !card) return null;
        return {
          id: note.id,
          anchor: mark.getBoundingClientRect().top - origin - cardInset,
          height: card.offsetHeight,
          y: 0,
        };
      })
      .filter((item) => item !== null)
      .sort((a, b) => a.anchor - b.anchor);
    const pin = placed.findIndex((item) => item.id === active);
    let floor = -Infinity;
    placed.forEach((item, index) => {
      item.y = index === pin ? item.anchor : Math.max(item.anchor, floor);
      floor = item.y + item.height + cardGap;
    });
    for (let index = pin - 1; index >= 0; index--)
      placed[index].y = Math.min(
        placed[index].y,
        placed[index + 1].y - placed[index].height - cardGap,
      );
    const next = Object.fromEntries(placed.map((item) => [item.id, item.y]));
    setTops((current) =>
      JSON.stringify(current) === JSON.stringify(next) ? current : next,
    );
    // Folding and reflow move marks without changing the notes, so they are listed too.
  }, [margin, notes, active, layoutTick, layer.focus, layer.openedRuns]);

  // Text reflows when the window or the fonts change, and a card grows while its note is being
  // written; the cards follow.
  const [resized] = useState(() =>
    typeof ResizeObserver === "undefined"
      ? null
      : new ResizeObserver(() => setLayoutTick((tick) => tick + 1)),
  );
  useEffect(() => {
    const root = area.current;
    if (!root) return;
    void document.fonts?.ready.then(() => setLayoutTick((tick) => tick + 1));
    resized?.observe(root);
    return () => resized?.disconnect();
  }, [resized]);

  // A floating note hangs under the last line of its mark.
  useLayoutEffect(() => {
    const root = area.current;
    if (margin || !floating || !root) return setFloatAt(null);
    const mark = marksOf(root, floating).at(-1);
    if (!mark) return setFloatAt(null);
    const rects = mark.getClientRects();
    const rect = rects.length
      ? rects[rects.length - 1]
      : mark.getBoundingClientRect();
    const left = Math.min(Math.max(16, rect.left), window.innerWidth - 316);
    setFloatAt({
      x: left + window.scrollX,
      y: rect.bottom + 8 + window.scrollY,
    });
  }, [margin, floating, layer.list, layoutTick]);

  const closeBar = () => setBar(null);

  function edit(id: string) {
    setEditing(id);
    setActive(id);
    // A wide window gets a margin as soon as there is a note to put in it.
    if (!wide) setFloating(id);
  }

  // A finished note is saved; an emptied one takes the note, or a note-only annotation, away.
  function finish(id: string, text: string) {
    setEditing(null);
    setFloating(null);
    if (id === draftId) {
      const draft = layer.draft;
      layer.setDraft(null);
      if (draft && text) void layer.create(draft, null, text);
      return;
    }
    const item = find(id);
    if (!item || text === (item.note ?? "")) return;
    void layer.update(id, item.style, text || null);
  }

  function applyStyle(style: AnnotationStyle) {
    if (!bar) return;
    if (bar.kind === "selection") {
      void layer.create(bar.anchor, style, null);
      document.getSelection()?.removeAllRanges();
    } else {
      const item = find(bar.id);
      // Pressing the current style again takes it off; nothing left means no annotation.
      if (item)
        void layer.update(
          item.id,
          item.style === style ? null : style,
          item.note,
        );
    }
    closeBar();
  }

  function note() {
    if (!bar) return;
    if (bar.kind === "selection") {
      layer.setDraft(bar.anchor);
      document.getSelection()?.removeAllRanges();
      edit(draftId);
    } else {
      edit(bar.id);
    }
    closeBar();
  }

  function removeTarget() {
    if (bar?.kind !== "annotation") return;
    void layer.remove(bar.id);
    closeBar();
  }

  function openSelection(selected: SelectedText) {
    setBar({
      kind: "selection",
      anchor: {
        scope: selected.scope,
        start: selected.start,
        end: selected.end,
        quote: selected.quote,
      },
      at: barPoint(selected.range.getBoundingClientRect?.() ?? nowhere),
    });
  }

  // A selection made anywhere ends with mouseup; a click on a mark is handled by the click.
  useEffect(() => {
    function onMouseUp(event: MouseEvent) {
      const target = event.target as Element | null;
      if (target?.closest?.(".anno-bar, .note-card")) return;
      setTimeout(() => {
        const root = area.current;
        const selected = root ? selectedText(root) : null;
        if (selected) openSelection(selected);
        else if (!target?.closest?.("mark.anno")) {
          setBar(null);
          if (!editing) setActive(null);
        }
      });
    }
    function onKeyUp(event: KeyboardEvent) {
      if (!event.shiftKey || !event.key.startsWith("Arrow")) return;
      const selected = area.current ? selectedText(area.current) : null;
      if (selected) openSelection(selected);
    }
    document.addEventListener("mouseup", onMouseUp);
    document.addEventListener("keyup", onKeyUp);
    return () => {
      document.removeEventListener("mouseup", onMouseUp);
      document.removeEventListener("keyup", onKeyUp);
    };
  });

  // The bar's keys while it shows; otherwise `a` flips 只看标注 and Escape leaves it.
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (
        event.defaultPrevented ||
        event.metaKey ||
        event.ctrlKey ||
        event.altKey
      )
        return;
      if (editing || typing(event.target)) return;
      if (bar) {
        const style = styleKeys[event.key];
        if (style) applyStyle(style);
        else if (event.key === "c") note();
        else if (
          bar.kind === "annotation" &&
          (event.key === "Delete" || event.key === "Backspace")
        )
          removeTarget();
        else if (event.key === "Escape") {
          closeBar();
          document.getSelection()?.removeAllRanges();
        } else return;
        event.preventDefault();
        return;
      }
      // A dialog or an open menu owns the keyboard.
      if (document.querySelector('[role="dialog"], [role="menu"]')) return;
      if (event.key === "a") {
        event.preventDefault();
        layer.toggleFocus();
      } else if (event.key === "Escape" && layer.focus) {
        layer.exitFocus();
      }
    }
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  });

  // Hovering a noted mark lights its note; on a narrow window it also floats the note out.
  function onMouseOver(event: ReactMouseEvent) {
    const mark = (event.target as Element).closest<HTMLElement>(
      "mark.anno[data-note]",
    );
    const id = mark?.dataset.annotation;
    if (!id || id === draftId) return;
    setHot(id);
    if (!margin && !editing) {
      clearTimeout(hoverTimer.current);
      hoverTimer.current = setTimeout(
        () => setFloating(id),
        floating ? 0 : 180,
      );
    }
  }
  function onMouseOut(event: ReactMouseEvent) {
    const mark = (event.target as Element).closest("mark.anno[data-note]");
    if (!mark || mark.contains(event.relatedTarget as Node | null)) return;
    if (!editing) setHot(null);
    clearTimeout(hoverTimer.current);
    if (!editing) hoverTimer.current = setTimeout(() => setFloating(null), 220);
  }

  function onClick(event: ReactMouseEvent) {
    const target = event.target as Element;
    const gap = target.closest<HTMLElement>("[data-fold-run]");
    if (gap) {
      const scope = gap.closest<HTMLElement>("[data-annotation-scope]")?.dataset
        .annotationScope;
      if (scope) layer.open(scope, Number(gap.dataset.foldRun));
      return;
    }
    const mark = target.closest<HTMLElement>("mark.anno");
    const id = mark?.dataset.annotation;
    if (!mark || !id || id === draftId || !document.getSelection()?.isCollapsed)
      return;
    setActive(id);
    setBar({
      kind: "annotation",
      id,
      at: barPoint(mark.getClientRects()[0] ?? mark.getBoundingClientRect()),
    });
  }

  const barItem = bar?.kind === "annotation" ? find(bar.id) : undefined;
  const floatingNote = notes.find((item) => item.id === floating);
  const cardProps = (item: (typeof notes)[number]) => ({
    style: item.style,
    note: item.note,
    editing: editing === item.id,
    hot: hot === item.id || editing === item.id,
    onEdit: () => edit(item.id),
    onFinish: (text: string) => finish(item.id, text),
  });
  return (
    <div
      ref={area}
      className={layer.focus ? "annotated focus" : "annotated"}
      onMouseOver={onMouseOver}
      onMouseOut={onMouseOut}
      onClick={onClick}
    >
      {children}
      {margin && (
        <div className="margin-notes">
          {notes.map((item) => (
            <NoteCard
              key={item.id}
              {...cardProps(item)}
              className="in-margin"
              cardRef={(element) => {
                if (element) {
                  cards.current.set(item.id, element);
                  resized?.observe(element);
                } else {
                  cards.current.delete(item.id);
                }
              }}
              position={
                item.id in tops
                  ? { top: tops[item.id] }
                  : { top: 0, visibility: "hidden" }
              }
              onHover={(inside) => {
                if (!editing) setHot(inside ? item.id : null);
              }}
            />
          ))}
        </div>
      )}
      {!margin &&
        floatingNote &&
        floatAt &&
        createPortal(
          <NoteCard
            {...cardProps(floatingNote)}
            className="floating"
            position={{ left: floatAt.x, top: floatAt.y }}
            onHover={(inside) => {
              clearTimeout(hoverTimer.current);
              if (!inside && !editing)
                hoverTimer.current = setTimeout(() => {
                  setFloating(null);
                  setHot(null);
                }, 220);
            }}
          />,
          document.body,
        )}
      {bar && (bar.kind === "selection" || barItem) && (
        <AnnotationBar
          subject={
            bar.kind === "selection"
              ? { kind: "selection" }
              : {
                  kind: "annotation",
                  style: barItem!.style,
                  noted: barItem!.note !== null,
                }
          }
          at={bar.at}
          onStyle={applyStyle}
          onNote={note}
          onRemove={removeTarget}
        />
      )}
      {layer.error && (
        <p role="status" className="annotation-error">
          {layer.error}
        </p>
      )}
    </div>
  );
}
