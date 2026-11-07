import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ApiError,
  overallNoteKey,
  overallNoteOptions,
  saveOverallNote,
  type OverallNoteTarget,
} from "@/lib/api";
import { MarkdownContent, type ImagePolicy } from "./markdown-content";
import { useShortcutHelp, type ShortcutGroup } from "./shortcut-help";

// How long typing must pause before what was written is saved.
const SAVE_AFTER_MS = 800;

const shortcuts: ShortcutGroup = {
  title: "总批",
  shortcuts: [{ keys: ["n"], action: "写总批" }],
};

const typing = (target: EventTarget | null) =>
  target instanceof Element &&
  target.closest("input, textarea, select, [contenteditable='true']") !== null;

// 总批: one block of text on the whole document, in a card pinned at the foot of the margin. It
// never enters the text flow, and a document without one shows nothing at all; `n` writes one.
// Mount it once per document, keyed by it: a card holds the text of the document it opened on.
export function OverallNote({
  target,
  images,
}: {
  target: OverallNoteTarget;
  // The document's own rule for images, so a note cannot load what the page would not.
  images: ImagePolicy;
}) {
  const client = useQueryClient();
  const note = useQuery(overallNoteOptions(target));
  const stored = note.data?.text ?? "";
  const [mode, setMode] = useState<"folded" | "reading" | "writing">("folded");
  const [draft, setDraft] = useState("");
  const [problem, setProblem] = useState<string | null>(null);
  useShortcutHelp(shortcuts);

  // What a save needs, read when it runs rather than when it was scheduled: the text may have
  // grown, and the save before it moved the version on.
  const latest = useRef({ draft, stored, version: note.data?.version ?? 0 });
  useEffect(() => {
    latest.current.draft = draft;
  }, [draft]);
  useEffect(() => {
    if (!note.data) return;
    latest.current.stored = note.data.text ?? "";
    latest.current.version = note.data.version;
  }, [note.data]);

  // Saves run one at a time, each naming the version the last one produced. It resolves to
  // whether Palace now has everything written.
  const queue = useRef(Promise.resolve(true));
  const save = () => {
    queue.current = queue.current.then(async () => {
      const { draft: text, stored: kept, version } = latest.current;
      if (text.trim() === kept.trim()) return true;
      try {
        const saved = await saveOverallNote(target, text, version);
        latest.current.stored = saved.text ?? "";
        latest.current.version = saved.version;
        client.setQueryData(overallNoteKey(target), saved);
        setProblem(null);
        return true;
      } catch (error) {
        setProblem(
          error instanceof ApiError && error.status === 409
            ? "总批在别处改过了，这里写的还没有保存。"
            : "没有保存，写下的内容还在这里。",
        );
        return false;
      }
    });
    return queue.current;
  };
  const saveRef = useRef(save);

  const pause = useRef<ReturnType<typeof setTimeout>>(undefined);
  const write = () => {
    setDraft(stored);
    latest.current.draft = stored;
    setMode("writing");
  };
  const edit = (text: string) => {
    setDraft(text);
    clearTimeout(pause.current);
    pause.current = setTimeout(() => void saveRef.current(), SAVE_AFTER_MS);
  };
  // The card folds only once Palace has the text, so a failed save stays on screen.
  const finish = async () => {
    clearTimeout(pause.current);
    if (await save()) setMode("folded");
  };

  // Leaving the page must not lose the last words typed before the pause ran out.
  useEffect(
    () => () => {
      clearTimeout(pause.current);
      void saveRef.current();
    },
    [],
  );

  const writeRef = useRef(write);
  useEffect(() => {
    saveRef.current = save;
    writeRef.current = write;
  });
  const loaded = note.isSuccess;
  useEffect(() => {
    function onKeyDown(event: globalThis.KeyboardEvent) {
      if (
        event.defaultPrevented ||
        event.metaKey ||
        event.ctrlKey ||
        event.altKey ||
        typing(event.target)
      )
        return;
      // A dialog, a menu or the annotation bar owns the keyboard.
      if (
        document.querySelector(
          '[role="dialog"], [role="menu"], [role="toolbar"]',
        )
      )
        return;
      if (event.key === "n" && loaded) {
        event.preventDefault();
        writeRef.current();
      } else if (event.key === "Escape")
        setMode((now) => (now === "reading" ? "folded" : now));
    }
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [loaded]);

  // Reading folds away as soon as attention moves on; writing folds on the textarea's blur.
  const card = useRef<HTMLElement>(null);
  useEffect(() => {
    if (mode !== "reading") return;
    function onPointerDown(event: PointerEvent) {
      if (!card.current?.contains(event.target as Node)) setMode("folded");
    }
    document.addEventListener("pointerdown", onPointerDown);
    return () => document.removeEventListener("pointerdown", onPointerDown);
  }, [mode]);

  function onEditorKeys(event: KeyboardEvent) {
    if (
      event.key === "Escape" ||
      (event.key === "Enter" && (event.metaKey || event.ctrlKey))
    ) {
      event.preventDefault();
      (event.target as HTMLElement).blur();
    }
  }

  if (mode === "folded" && !stored) return null;
  return (
    <aside
      ref={card}
      className={mode === "folded" ? "overall-note" : "overall-note open"}
      aria-label="总批"
    >
      {mode === "folded" && (
        <button
          type="button"
          className="overall-note-fold"
          onClick={() => setMode("reading")}
        >
          <span className="overall-note-name">总批</span>
          <span className="overall-note-first">{stored.split("\n")[0]}</span>
        </button>
      )}
      {mode !== "folded" && (
        <div className="overall-note-open">
          <span className="overall-note-name">总批</span>
          {mode === "writing" ? (
            <textarea
              aria-label="总批"
              autoFocus
              // New thoughts go after the old ones.
              onFocus={({ target: { value }, target: editor }) =>
                editor.setSelectionRange(value.length, value.length)
              }
              value={draft}
              onChange={(event) => edit(event.target.value)}
              onKeyDown={onEditorKeys}
              onBlur={() => void finish()}
            />
          ) : (
            <div
              className="overall-note-text"
              onClick={(event) => {
                if (!(event.target as Element).closest("a")) write();
              }}
            >
              <MarkdownContent content={stored} images={images} />
            </div>
          )}
          {problem && (
            <p role="alert" className="overall-note-problem">
              {problem}
            </p>
          )}
        </div>
      )}
    </aside>
  );
}
