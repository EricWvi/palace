import { createPortal } from "react-dom";
import { highlightStyles, type AnnotationStyle } from "@/lib/annotation-paint";

// Four highlighter grounds, then two pen strokes, each with the key that picks it.
export const styleKeys: Record<string, AnnotationStyle> = {
  "1": "yellow",
  "2": "green",
  "3": "blue",
  "4": "rose",
  h: "yellow",
  u: "underline",
  w: "wavy",
};
const labels: Record<AnnotationStyle, string> = {
  yellow: "黄色高亮（1）",
  green: "绿色高亮（2）",
  blue: "蓝色高亮（3）",
  rose: "红色高亮（4）",
  underline: "划线（u）",
  wavy: "波浪线（w）",
};

// What the bar acts on: text just selected, or an annotation that was clicked.
export type BarSubject =
  | { kind: "selection" }
  | { kind: "annotation"; style: AnnotationStyle | null; noted: boolean };

// The small bar over a selection or a clicked mark. It never takes focus: pressing a button keeps
// the reader's selection, and the keys it lists work while it shows.
export function AnnotationBar({
  subject,
  at,
  onStyle,
  onNote,
  onRemove,
}: {
  subject: BarSubject;
  // Page coordinates of the point the bar's bottom edge centres on.
  at: { x: number; y: number };
  onStyle: (style: AnnotationStyle) => void;
  onNote: () => void;
  onRemove: () => void;
}) {
  const current = subject.kind === "annotation" ? subject.style : null;
  const pressed = (style: AnnotationStyle) =>
    subject.kind === "annotation" ? style === current : undefined;
  return createPortal(
    <div
      className="anno-bar"
      role="toolbar"
      aria-label="标注"
      style={{ left: at.x, top: at.y }}
      onMouseDown={(event) => event.preventDefault()}
    >
      {highlightStyles.map((style) => (
        <button
          key={style}
          type="button"
          className={`swatch ${style}`}
          aria-label={labels[style]}
          title={labels[style]}
          aria-pressed={pressed(style)}
          onClick={() => onStyle(style)}
        />
      ))}
      <span className="rule" />
      {(["underline", "wavy"] as const).map((style) => (
        <button
          key={style}
          type="button"
          className={`pen ${style}`}
          aria-label={labels[style]}
          title={labels[style]}
          aria-pressed={pressed(style)}
          onClick={() => onStyle(style)}
        >
          文
        </button>
      ))}
      <span className="rule" />
      <button type="button" className="word" title="批注（c）" onClick={onNote}>
        {subject.kind === "annotation" && subject.noted ? "改批注" : "批注"}
      </button>
      {subject.kind === "annotation" && (
        <>
          <span className="rule" />
          <button
            type="button"
            className="word remove"
            title="删除（Delete）"
            onClick={onRemove}
          >
            删除
          </button>
        </>
      )}
    </div>,
    document.body,
  );
}
