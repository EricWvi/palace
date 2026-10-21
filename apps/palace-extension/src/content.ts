// Runs in every web page. On a page not yet bound it lets the reader annotate as in Palace and
// draws the page's annotations; on a bound page it draws the article's current annotations,
// read only. It holds no data of its own: every change goes straight to Palace through the
// service worker, and a page that cannot reach Palace offers nothing (D3, D6, D7).
import {
  findQuote,
  relocate,
  type Located,
} from "../../palace-web/src/lib/text-quote";
import { paint, type Look } from "./highlights";
import {
  Overlay,
  styleKeys,
  type BarActions,
  type BarSubject,
} from "./overlay";
import { pageText, rangeOf, selectionQuote } from "./page-text";
import {
  pageAddress,
  send,
  type AnnotationStyle,
  type PageState,
  type Reply,
  type WebAnnotation,
} from "./protocol";

// One annotation as drawn now, with the range its words occupy on the page.
type Drawn = {
  id: string;
  range: Range;
  style: AnnotationStyle | null;
  note: string | null;
};

const overlay = new Overlay();
let state: PageState | null = null;
let drawn: Drawn[] = [];
// The selection the bar was opened for, kept as a selector so a lost selection loses nothing.
let selected: ReturnType<typeof selectionQuote> = null;
// What the open bar does, for the keys it lists.
let bar: { subject: BarSubject; actions: BarActions } | null = null;

function showBar(
  at: { x: number; y: number },
  subject: BarSubject,
  actions: BarActions,
) {
  overlay.showBar(at, subject, actions);
  bar = { subject, actions };
}

function hideBar() {
  overlay.hideBar();
  bar = null;
}

const lookOf = (style: AnnotationStyle | null): Look => style ?? "plain";
const address = () => pageAddress(location.href);

// Finds every annotation's words on the page as it is now and draws them.
function draw() {
  const page = pageText(document.body);
  drawn = [];
  if (state?.kind === "unbound") {
    for (const annotation of state.annotations) {
      const at = relocate(page.text, annotation);
      if (at) drawn.push(mark(annotation, at));
    }
  } else if (state?.kind === "bound") {
    const length = Math.max(state.article.length, 1);
    for (const annotation of state.annotations) {
      const at = findQuote(
        page.text,
        annotation.quote,
        annotation.start / length,
      );
      if (at) drawn.push(mark(annotation, at));
    }
  }
  paint(drawn.map((item) => ({ range: item.range, look: lookOf(item.style) })));

  function mark(
    annotation: {
      id: string;
      style: AnnotationStyle | null;
      note: string | null;
    },
    at: Located,
  ): Drawn {
    return {
      id: annotation.id,
      range: rangeOf(page, at.start, at.end),
      style: annotation.style,
      note: annotation.note,
    };
  }
}

// Fetches what this page is and draws it; a page that cannot reach Palace shows nothing.
async function load() {
  const reply = await send({ kind: "page", url: address() });
  state = reply.ok ? reply.data : null;
  hideBar();
  overlay.hideNote();
  draw();
}

// The annotation under a viewport point, if any.
function hit(x: number, y: number): Drawn | null {
  for (const item of drawn)
    for (const rect of item.range.getClientRects())
      if (
        x >= rect.left &&
        x <= rect.right &&
        y >= rect.top &&
        y <= rect.bottom
      )
        return item;
  return null;
}

// Applies a reply: a failure is said once; a page bound meanwhile elsewhere turns read only.
function settled<T>(reply: Reply<T>, apply: (data: T) => void) {
  if (reply.ok) {
    apply(reply.data);
    draw();
    return;
  }
  overlay.flash(`标注没有保存：${reply.message}`);
  if (reply.problem === "bound") void load();
}

function own(): WebAnnotation[] {
  return state?.kind === "unbound" ? state.annotations : [];
}

async function create(style: AnnotationStyle | null, note: string | null) {
  const quote = selected;
  if (!quote) return;
  hideBar();
  getSelection()?.removeAllRanges();
  const reply = await send({
    kind: "create",
    annotation: {
      url: address(),
      title: document.title,
      quote: quote.quote,
      prefix: quote.prefix,
      suffix: quote.suffix,
      position: quote.position,
      style,
      note,
    },
  });
  settled(reply, (created) => own().push(created));
}

// Replaces the style and the note together; with neither left, the annotation goes.
async function change(
  item: Drawn,
  style: AnnotationStyle | null,
  note: string | null,
) {
  hideBar();
  if (style === null && note === null) return remove(item);
  const reply = await send({
    kind: "update",
    id: item.id,
    change: { title: document.title, style, note },
  });
  settled(reply, (updated) => {
    if (state?.kind === "unbound")
      state.annotations = own().map((annotation) =>
        annotation.id === updated.id ? updated : annotation,
      );
  });
}

async function remove(item: Drawn) {
  hideBar();
  const reply = await send({ kind: "delete", id: item.id });
  settled(reply, () => {
    if (state?.kind === "unbound")
      state.annotations = own().filter(
        (annotation) => annotation.id !== item.id,
      );
  });
}

// Opens a note editor under `under`; blank words write nothing.
function editNote(
  under: DOMRect,
  text: string,
  style: AnnotationStyle | null,
  finish: (text: string | null) => void,
) {
  hideBar();
  overlay.showNote(under, {
    text,
    style,
    editing: true,
    onFinish: (written) => finish(written.trim() === "" ? null : written),
  });
}

function lastRect(range: Range) {
  const rects = range.getClientRects();
  return rects[rects.length - 1] ?? range.getBoundingClientRect();
}

// Opens the bar for a clicked annotation.
function openAnnotation(item: Drawn) {
  const first =
    item.range.getClientRects()[0] ?? item.range.getBoundingClientRect();
  overlay.hideNote();
  showBar(
    { x: first.left + first.width / 2, y: first.top - 8 },
    { kind: "annotation", style: item.style, noted: item.note !== null },
    {
      // Picking the current style again takes it off.
      onStyle: (style) =>
        void change(item, style === item.style ? null : style, item.note),
      onNote: () =>
        editNote(
          lastRect(item.range),
          item.note ?? "",
          item.style,
          (note) => void change(item, item.style, note),
        ),
      onRemove: () => void remove(item),
    },
  );
}

document.addEventListener("mouseup", (event) => {
  if (state?.kind !== "unbound" || overlay.owns(event)) return;
  // Let the selection settle before reading it.
  setTimeout(() => {
    const selection = getSelection();
    const range =
      selection && selection.rangeCount > 0 ? selection.getRangeAt(0) : null;
    selected = range ? selectionQuote(document.body, range) : null;
    if (selected && range) {
      const rect = range.getBoundingClientRect();
      showBar(
        { x: rect.left + rect.width / 2, y: rect.top - 8 },
        { kind: "selection" },
        {
          onStyle: (style) => void create(style, null),
          onNote: () =>
            editNote(lastRect(range), "", null, (note) => {
              if (note !== null) void create(null, note);
            }),
          // A selection has nothing to delete; its bar shows no such button.
          onRemove: () => {},
        },
      );
      return;
    }
    const item = hit(event.clientX, event.clientY);
    if (item) openAnnotation(item);
    else hideBar();
  });
});

// Hovering a noted mark shows its note under it; on an unbound page a click on it edits.
let pending = 0;
document.addEventListener("mousemove", (event) => {
  if (overlay.noteEditing || overlay.barShown || overlay.owns(event)) return;
  cancelAnimationFrame(pending);
  pending = requestAnimationFrame(() => {
    const item = hit(event.clientX, event.clientY);
    if (!item || item.note === null) return overlay.hideNote();
    const editable = state?.kind === "unbound";
    overlay.showNote(lastRect(item.range), {
      text: item.note,
      style: item.style,
      editing: false,
      onEdit: editable
        ? () =>
            editNote(
              lastRect(item.range),
              item.note ?? "",
              item.style,
              (note) => void change(item, item.style, note),
            )
        : undefined,
    });
  });
});

// The keys the bar lists, while it shows.
document.addEventListener(
  "keydown",
  (event) => {
    if (!bar || event.metaKey || event.ctrlKey || event.altKey) return;
    const style = styleKeys[event.key];
    if (style) bar.actions.onStyle(style);
    else if (event.key === "c") bar.actions.onNote();
    else if (event.key === "Escape") hideBar();
    else if (
      bar.subject.kind === "annotation" &&
      (event.key === "Delete" || event.key === "Backspace")
    )
      bar.actions.onRemove();
    else return;
    event.preventDefault();
    event.stopPropagation();
  },
  true,
);

// Positions are those of the moment; scrolling moves the words away from the bar.
addEventListener(
  "scroll",
  () => {
    hideBar();
    if (!overlay.noteEditing) overlay.hideNote();
  },
  { passive: true },
);

// Pages fill in after load and change as they are read; find the words again once they settle.
let settling = 0;
new MutationObserver(() => {
  clearTimeout(settling);
  settling = window.setTimeout(draw, 400);
}).observe(document.body, {
  childList: true,
  subtree: true,
  characterData: true,
});

// The popup asks again after binding the page.
chrome.runtime.onMessage.addListener((message: { kind: string }) => {
  if (message.kind === "refresh") void load();
});

void load();
