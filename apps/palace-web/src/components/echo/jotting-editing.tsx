import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  ApiError,
  deleteJotting,
  editJotting,
  patchJotting,
  type ListedJotting,
} from "@/lib/api";
import { fitTextarea } from "@/lib/fit-textarea";
import { parseDraft, writeTag } from "@/lib/jotting-draft";

// How long 确认 waits for the second click before it is 削简 again.
const CONFIRM_MS = 3000;

// The draft of an open edit: the title, and the body with the tags written back at its end.
type Draft = { title: string; text: string };

// The slips a zoomed card offers of its own, for the caller to place among its others.
export type OwnSlips = { edit: ReactNode; remove: ReactNode };

// Editing and deleting a zoomed jotting. 修订 turns the card into its own editor: the title in
// one line, the body below it with the tags written back as `#tag` at its end, taken out again
// on saving the way a new jotting's are. 削简 turns into 确认 and deletes only on a second
// click on the same slip, within three seconds of the first and before it loses focus. The
// pointer leaving the slip does not disarm it: one wait serves both.
// See specs/decisions/web/echo/20261101-edit-and-delete-a-zoomed-jotting-with-title-slips.md.
export function useJottingEditing(
  jotting: ListedJotting,
  onDeleted: () => void,
) {
  const client = useQueryClient();
  // The draft as it opened, to tell whether anything was changed.
  const [start, setStart] = useState<Draft | null>(null);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [confirming, setConfirming] = useState(false);
  const changed =
    draft !== null &&
    (draft.title !== start?.title || draft.text !== start?.text);

  const save = useMutation({
    mutationFn: (edit: {
      title: string | null;
      tags: string[];
      body: string;
    }) => editJotting(jotting.id, jotting.version, edit),
    onSuccess: ({ version }, edit) => {
      patchJotting(client, jotting.id, { ...edit, version, pending: true });
      setDraft(null);
    },
  });
  const remove = useMutation({
    mutationFn: () => deleteJotting(jotting.id, jotting.version),
    onSuccess: onDeleted,
  });

  // A first click is forgotten after a while, so a much later click never deletes.
  useEffect(() => {
    if (!confirming) return;
    const timer = window.setTimeout(() => setConfirming(false), CONFIRM_MS);
    return () => window.clearTimeout(timer);
  }, [confirming]);

  function open() {
    const tags = jotting.tags.map(writeTag).join(" ");
    const text = tags ? `${jotting.body.trimEnd()}\n\n${tags}` : jotting.body;
    const opened = { title: jotting.title ?? "", text };
    save.reset();
    setStart(opened);
    setDraft(opened);
  }

  function submit() {
    if (!draft || save.isPending || conflicted(save.error)) return;
    const { tags, body } = parseDraft(draft.text);
    const title = draft.title.trim() || null;
    // Nothing the card shows would change: back to reading, without asking the server.
    if (
      title === (jotting.title ?? null) &&
      body === jotting.body &&
      tags.join("\n") === jotting.tags.join("\n")
    ) {
      setDraft(null);
      return;
    }
    save.mutate({ title, tags, body });
  }

  // Leaves an edit without changes; an edit with changes leaves only by 取消 or by saving.
  function leave() {
    if (!changed) setDraft(null);
  }

  // Keys inside the editor's fields, before the card's own Escape sees them.
  function onFieldKey(event: KeyboardEvent) {
    if (event.nativeEvent.isComposing) return;
    if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
      event.preventDefault();
      submit();
    } else if (event.key === "Escape") {
      event.stopPropagation();
      leave();
    }
  }

  const failure = draft ? save.error : remove.error;
  const notice = failure
    ? conflicted(failure)
      ? "札记已在别处改过，复制修改后取消再修订"
      : draft
        ? "保存失败"
        : "删除失败"
    : null;

  const fields = draft && (
    <div className="jot-editor">
      <input
        aria-label="标题"
        placeholder="标题"
        value={draft.title}
        onChange={(event) => setDraft({ ...draft, title: event.target.value })}
        onKeyDown={(event) => {
          onFieldKey(event);
          if (event.key === "Enter" && !event.metaKey && !event.ctrlKey)
            event.preventDefault();
        }}
      />
      <Body
        value={draft.text}
        onChange={(text) => setDraft({ ...draft, text })}
        onKeyDown={onFieldKey}
      />
    </div>
  );

  const busy = save.isPending || remove.isPending;
  const own: OwnSlips = {
    edit: (
      <button key="edit" type="button" disabled={busy} onClick={open}>
        修订
      </button>
    ),
    remove: (
      <button
        key="remove"
        type="button"
        className={confirming ? "confirming" : undefined}
        disabled={busy}
        onClick={() => {
          if (!confirming) setConfirming(true);
          else {
            setConfirming(false);
            remove.mutate();
          }
        }}
        onBlur={() => setConfirming(false)}
      >
        {confirming ? "确认" : "削简"}
      </button>
    ),
  };
  const editingSlips = [
    <button
      key="save"
      type="button"
      disabled={busy || conflicted(save.error)}
      onClick={submit}
    >
      保存
    </button>,
    <button
      key="cancel"
      type="button"
      disabled={busy}
      onClick={() => setDraft(null)}
    >
      取消
    </button>,
  ];

  return {
    editing: draft !== null,
    leave,
    fields,
    own,
    editingSlips,
    notice,
    busy,
  };
}

// The body field grows with its text; past the card's cap the card scrolls instead.
function Body({
  value,
  onChange,
  onKeyDown,
}: {
  value: string;
  onChange: (text: string) => void;
  onKeyDown: (event: KeyboardEvent) => void;
}) {
  const field = useRef<HTMLTextAreaElement>(null);
  // Opens with the caret at the end of the words, before the tags written back after them.
  useEffect(() => {
    const element = field.current;
    if (!element) return;
    element.focus({ preventScroll: true });
    const tags = element.value.match(/\n\n#[^\s#][^\n]*$/);
    const caret = tags ? tags.index! : element.value.length;
    element.setSelectionRange(caret, caret);
  }, []);
  useLayoutEffect(() => {
    if (field.current) fitTextarea(field.current);
  }, [value]);
  return (
    <textarea
      ref={field}
      aria-label="正文"
      value={value}
      onChange={(event) => onChange(event.target.value)}
      onKeyDown={onKeyDown}
    />
  );
}

// The jotting changed elsewhere, or its file is moving: saving again cannot succeed.
function conflicted(error: unknown) {
  return error instanceof ApiError && error.status === 409;
}
