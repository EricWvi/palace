// The extension's interface on a web page: the bar over a selection or a clicked mark, a note
// card, and a one-line message. It lives in a closed shadow root on its own element outside the
// body, so the page's CSS cannot restyle it, its scripts cannot read it, and the page text the
// marks anchor in never includes it. Its look follows Palace's annotation bar and note cards.
import tokens from "../../../design/tokens.css?raw";
import type { AnnotationStyle } from "./protocol";

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

export type BarActions = {
  onStyle: (style: AnnotationStyle) => void;
  onNote: () => void;
  onRemove: () => void;
};

// A note under its mark: read only on a bound page, otherwise edited in place and handed back
// when the reader leaves it, presses Escape or ⌘/Ctrl+Enter.
export type NoteCard = {
  text: string;
  style: AnnotationStyle | null;
  editing: boolean;
  onEdit?: () => void;
  onFinish?: (text: string) => void;
};

// The card's left rule takes its mark's colour, as in Palace.
function swatch(style: AnnotationStyle | null) {
  switch (style) {
    case null:
      return "var(--accent)";
    case "underline":
      return "var(--mark-line)";
    case "wavy":
      return "var(--mark-wave)";
    case "yellow":
    case "green":
    case "blue":
    case "rose":
      return `color-mix(in srgb, var(--mark-${style}), var(--text) 28%)`;
  }
}

const css = `
${tokens.replaceAll(":root", ":host")}
:host { all: initial; }
* { box-sizing: border-box; margin: 0; }
button { all: unset; cursor: pointer; }
.bar {
  position: fixed; z-index: 2147483647; display: flex; align-items: center; gap: 2px;
  padding: 4px; background: var(--surface-raised); border: 1px solid var(--border);
  border-radius: var(--radius-md); box-shadow: 0 6px 20px rgb(0 0 0 / 0.1);
  font-family: var(--font-ui); font-size: var(--font-size-sm); line-height: 1; white-space: nowrap;
  transform: translate(-50%, -100%);
}
.bar button {
  display: grid; place-items: center; min-width: 28px; height: 28px;
  border-radius: var(--radius-sm); color: var(--text-2);
}
.bar button:hover { background: var(--bg-sunken); color: var(--text); }
.swatch::before {
  content: ""; width: 14px; height: 14px; border-radius: var(--radius-round); background: var(--m);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--text) 12%, transparent);
}
.swatch[aria-pressed="true"]::before {
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--text) 12%, transparent),
    0 0 0 2px var(--surface-raised), 0 0 0 3.5px var(--text-3);
}
.yellow { --m: var(--mark-yellow); }
.green { --m: var(--mark-green); }
.blue { --m: var(--mark-blue); }
.rose { --m: var(--mark-rose); }
.pen { font-size: var(--font-size-md); text-decoration-skip-ink: none; }
.pen.underline { text-decoration: underline 2px var(--mark-line); text-underline-offset: 4px; }
.pen.wavy { text-decoration: underline wavy 1.5px var(--mark-wave); text-underline-offset: 3px; }
.pen[aria-pressed="true"] { background: var(--bg-sunken); color: var(--text); }
.word { padding: 0 8px; }
.remove:hover { color: var(--accent); }
.rule { width: 1px; height: 16px; margin: 0 4px; background: var(--border); }
.card {
  position: fixed; z-index: 2147483646; width: 280px; padding: 9px 14px 10px 16px;
  background: var(--surface-raised); border: 1px solid var(--border); border-radius: var(--radius-md);
  box-shadow: 0 2px 6px rgb(0 0 0 / 0.07), 0 12px 32px rgb(0 0 0 / 0.13);
  color: var(--text); font-family: var(--font-ui); font-size: var(--font-size-sm); line-height: 1.65;
  white-space: pre-wrap; overflow-wrap: anywhere;
}
.card::before {
  content: ""; position: absolute; top: 9px; bottom: 9px; left: -1px; width: 2px;
  border-radius: 1px; background: var(--swatch);
}
.card.editable { cursor: text; }
.card textarea {
  all: unset; display: block; width: 100%; min-height: 3.3em; white-space: pre-wrap;
  overflow-wrap: anywhere; field-sizing: content;
}
.card textarea::placeholder { color: var(--text-3); }
.message {
  position: fixed; left: 50%; bottom: 24px; z-index: 2147483647; padding: 8px 14px;
  background: var(--surface-raised); border: 1px solid var(--border); border-radius: var(--radius-md);
  box-shadow: 0 6px 20px rgb(0 0 0 / 0.1); color: var(--accent);
  font-family: var(--font-ui); font-size: var(--font-size-sm); transform: translateX(-50%);
}
`;

function element<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className = "",
  text = "",
): HTMLElementTagNameMap[K] {
  const made = document.createElement(tag);
  if (className) made.className = className;
  if (text) made.textContent = text;
  return made;
}

export class Overlay {
  readonly host: HTMLElement;
  private root: ShadowRoot;
  private bar: HTMLElement | null = null;
  private card: HTMLElement | null = null;
  private editing = false;
  private timer = 0;

  constructor() {
    this.host = document.createElement("palace-annotator");
    this.root = this.host.attachShadow({ mode: "closed" });
    this.root.append(element("style", "", css));
    document.documentElement.append(this.host);
  }

  // Whether an event happened on the overlay rather than the page.
  owns(event: Event) {
    return event.composedPath().includes(this.host);
  }

  get barShown() {
    return this.bar !== null;
  }

  get noteEditing() {
    return this.editing;
  }

  // Shows the bar with its bottom edge centred on `at`, in viewport coordinates.
  showBar(
    at: { x: number; y: number },
    subject: BarSubject,
    actions: BarActions,
  ) {
    this.hideBar();
    const current = subject.kind === "annotation" ? subject.style : null;
    const pressed = (style: AnnotationStyle) =>
      subject.kind === "annotation" ? String(style === current) : "";
    const button = (
      className: string,
      label: string,
      onClick: () => void,
      text = "",
    ) => {
      const made = element("button", className, text);
      made.title = label;
      made.setAttribute("aria-label", label);
      made.addEventListener("click", onClick);
      return made;
    };
    const bar = element("div", "bar");
    bar.setAttribute("role", "toolbar");
    bar.setAttribute("aria-label", "标注");
    bar.style.left = `${at.x}px`;
    bar.style.top = `${at.y}px`;
    // Pressing a button must not take the reader's selection away.
    bar.addEventListener("mousedown", (event) => event.preventDefault());
    for (const style of ["yellow", "green", "blue", "rose"] as const) {
      const made = button(`swatch ${style}`, labels[style], () =>
        actions.onStyle(style),
      );
      if (pressed(style)) made.setAttribute("aria-pressed", pressed(style));
      bar.append(made);
    }
    bar.append(element("span", "rule"));
    for (const style of ["underline", "wavy"] as const) {
      const made = button(
        `pen ${style}`,
        labels[style],
        () => actions.onStyle(style),
        "文",
      );
      if (pressed(style)) made.setAttribute("aria-pressed", pressed(style));
      bar.append(made);
    }
    bar.append(element("span", "rule"));
    const noted = subject.kind === "annotation" && subject.noted;
    bar.append(
      button("word", "批注（c）", actions.onNote, noted ? "改批注" : "批注"),
    );
    if (subject.kind === "annotation") {
      bar.append(element("span", "rule"));
      bar.append(
        button("word remove", "删除（Delete）", actions.onRemove, "删除"),
      );
    }
    this.root.append(bar);
    this.bar = bar;
  }

  hideBar() {
    this.bar?.remove();
    this.bar = null;
  }

  // Shows a note card under `under`, a mark's last line, kept inside the window.
  showNote(under: DOMRect, note: NoteCard) {
    this.hideNote();
    const card = element("div", note.onEdit ? "card editable" : "card");
    card.style.setProperty("--swatch", swatch(note.style));
    card.style.left = `${Math.max(8, Math.min(under.left, innerWidth - 288))}px`;
    card.style.top = `${under.bottom + 8}px`;
    this.editing = note.editing;
    if (note.editing) {
      const field = element("textarea");
      field.value = note.text;
      field.placeholder = "写下批注";
      field.setAttribute("aria-label", "批注");
      let done = false;
      const finish = () => {
        if (done) return;
        done = true;
        this.editing = false;
        note.onFinish?.(field.value);
        this.hideNote();
      };
      field.addEventListener("blur", finish);
      field.addEventListener("keydown", (event) => {
        event.stopPropagation();
        if (
          event.key === "Escape" ||
          (event.key === "Enter" && (event.metaKey || event.ctrlKey))
        ) {
          event.preventDefault();
          finish();
        }
      });
      card.append(field);
      this.root.append(card);
      field.focus();
    } else {
      card.textContent = note.text;
      if (note.onEdit) card.addEventListener("click", note.onEdit);
      this.root.append(card);
    }
    this.card = card;
  }

  hideNote() {
    this.card?.remove();
    this.card = null;
    this.editing = false;
  }

  // Says something once at the foot of the window, then goes.
  flash(text: string) {
    this.root.querySelector(".message")?.remove();
    clearTimeout(this.timer);
    const message = element("p", "message", text);
    message.setAttribute("role", "status");
    this.root.append(message);
    this.timer = window.setTimeout(() => message.remove(), 4000);
  }
}
