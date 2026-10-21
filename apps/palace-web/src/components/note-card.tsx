import { useLayoutEffect, useRef, type CSSProperties, type Ref } from "react";
import { markKind, type AnnotationStyle } from "@/lib/annotation-paint";

// The card's left rule takes its mark's colour; pale highlighter grounds are deepened to show.
function swatch(style: AnnotationStyle | null) {
  switch (markKind(style)) {
    case "hl":
      return `color-mix(in srgb, var(--mark-${style}), var(--text) 28%)`;
    case "line":
      return "var(--mark-line)";
    case "wave":
      return "var(--mark-wave)";
    case "plain":
      return "var(--accent)";
  }
}

// One note, in the margin or floating under its mark. Clicking it edits it in place; leaving it,
// Escape or ⌘/Ctrl+Enter hands the text back, which the page saves.
export function NoteCard({
  style,
  note,
  editing,
  hot,
  className,
  position,
  cardRef,
  onEdit,
  onFinish,
  onHover,
}: {
  style: AnnotationStyle | null;
  note: string;
  editing: boolean;
  hot: boolean;
  className: string;
  position?: CSSProperties;
  cardRef?: Ref<HTMLDivElement>;
  onEdit: () => void;
  onFinish: (text: string) => void;
  onHover: (inside: boolean) => void;
}) {
  const field = useRef<HTMLTextAreaElement>(null);
  // The field grows with its text; the caret starts after what is already written.
  useLayoutEffect(() => {
    const element = field.current;
    if (!element) return;
    element.style.height = "0";
    element.style.height = `${element.scrollHeight}px`;
  });
  useLayoutEffect(() => {
    const element = field.current;
    if (!editing || !element) return;
    element.focus();
    element.setSelectionRange(element.value.length, element.value.length);
  }, [editing]);
  const classes = [className, "note-card", hot && "hot", editing && "editing"]
    .filter(Boolean)
    .join(" ");
  return (
    <div
      ref={cardRef}
      className={classes}
      style={{ ...position, ["--swatch" as string]: swatch(style) }}
      onMouseEnter={() => onHover(true)}
      onMouseLeave={() => onHover(false)}
      onClick={() => {
        if (!editing) onEdit();
      }}
    >
      {editing ? (
        <textarea
          ref={field}
          className="note-text"
          aria-label="批注"
          placeholder="写下批注…"
          defaultValue={note}
          rows={1}
          onInput={(event) => {
            const element = event.currentTarget;
            element.style.height = "0";
            element.style.height = `${element.scrollHeight}px`;
          }}
          onBlur={(event) => onFinish(event.currentTarget.value.trim())}
          onKeyDown={(event) => {
            if (
              event.key === "Escape" ||
              (event.key === "Enter" && (event.metaKey || event.ctrlKey))
            ) {
              event.preventDefault();
              event.stopPropagation();
              event.currentTarget.blur();
            }
          }}
        />
      ) : (
        <p className="note-text">{note}</p>
      )}
    </div>
  );
}
