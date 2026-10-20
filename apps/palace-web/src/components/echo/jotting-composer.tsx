import { useEffect, useRef, useState } from "react";
import { flushSync } from "react-dom";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { v7 as uuidv7 } from "uuid";
import { api, apiData, refreshJottings } from "@/lib/api";
import {
  completeTag,
  parseDraft,
  suggestTags,
  tagFragment,
} from "@/lib/jotting-draft";

// Writes a new jotting. `#tag` typed in the text becomes a tag, with known tags offered for the
// fragment before the caret. Ctrl/Cmd+Enter saves; Escape or leaving the field puts away only an
// empty draft, so words are never lost to a stray key.
export function JottingComposer({
  open,
  knownTags,
  onClose,
  onCreated,
}: {
  open: boolean;
  // Most used first, the order suggestions keep.
  knownTags: string[];
  onClose: () => void;
  onCreated: () => void;
}) {
  const client = useQueryClient();
  const field = useRef<HTMLTextAreaElement>(null);
  const [text, setText] = useState("");
  const [suggestions, setSuggestions] = useState<string[]>([]);
  const [selected, setSelected] = useState(0);
  const save = useMutation({
    mutationFn: (draft: string) => {
      const { tags, body } = parseDraft(draft);
      return apiData(
        api.POST("/api/jottings", {
          body: { id: uuidv7(), created_at: Date.now(), tags, body },
        }),
      );
    },
    onSuccess: () => {
      setText("");
      refreshJottings(client);
      onCreated();
    },
  });

  // The field opens with focus, as the shortcut that opened it promised.
  useEffect(() => {
    if (open) field.current?.focus({ preventScroll: true });
  }, [open]);
  // The field grows with its text instead of scrolling inside itself.
  useEffect(() => {
    const element = field.current;
    if (!element) return;
    element.style.height = "auto";
    element.style.height = `${element.scrollHeight}px`;
  }, [text]);

  function suggestFor(value: string, caret: number) {
    const fragment = tagFragment(value, caret);
    setSuggestions(fragment === null ? [] : suggestTags(knownTags, fragment));
    setSelected(0);
  }

  function pick(tag: string) {
    const element = field.current;
    if (!element) return;
    const next = completeTag(text, element.selectionStart, tag);
    // The caret can move only once the new text is in the field.
    flushSync(() => {
      setText(next.text);
      setSuggestions([]);
    });
    element.setSelectionRange(next.caret, next.caret);
  }

  function submit() {
    if (parseDraft(text).body && !save.isPending) save.mutate(text);
  }

  return (
    <div className="composer echo-card">
      <textarea
        ref={field}
        aria-label="新札记"
        placeholder="此刻在想什么"
        value={text}
        onChange={(event) => {
          setText(event.target.value);
          suggestFor(event.target.value, event.target.selectionStart);
        }}
        onBlur={() => {
          if (!text.trim()) onClose();
        }}
        onKeyDown={(event) => {
          if (event.nativeEvent.isComposing) return;
          if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
            event.preventDefault();
            submit();
            return;
          }
          if (suggestions.length > 0) {
            if (event.key === "ArrowDown" || event.key === "ArrowUp") {
              event.preventDefault();
              const step = event.key === "ArrowDown" ? 1 : -1;
              setSelected(
                (selected + step + suggestions.length) % suggestions.length,
              );
              return;
            }
            if (event.key === "Enter" || event.key === "Tab") {
              event.preventDefault();
              pick(suggestions[selected]);
              return;
            }
            if (event.key === "Escape") {
              setSuggestions([]);
              return;
            }
          }
          if (event.key === "Escape" && !text.trim()) {
            event.currentTarget.blur();
            onClose();
          }
        }}
      />
      {suggestions.length > 0 && (
        <ul className="suggest" role="listbox" aria-label="已有标签">
          {suggestions.map((tag, i) => (
            <li
              key={tag}
              role="option"
              aria-selected={i === selected}
              // Picking must not blur the field, or an empty draft would close under the pointer.
              onMouseDown={(event) => {
                event.preventDefault();
                pick(tag);
              }}
            >
              #{tag}
            </li>
          ))}
        </ul>
      )}
      {save.isError && (
        <p className="composer-error" role="alert">
          {save.error.message}
        </p>
      )}
    </div>
  );
}
