import { useState, type ClipboardEvent, type RefObject } from "react";
import { flushSync } from "react-dom";
import { api, apiData } from "@/lib/api";
import {
  imageMarkdown,
  imageName,
  pastedImages,
  placeholder,
  shiftCaret,
} from "@/lib/pasted-images";

// Each upload's placeholder names it; the counter is shared so two fields never reuse one.
let uploads = 0;

// Pasting pictures into a jotting field: each image file on the clipboard is uploaded to
// Palace media and written where the caret was as `![name](/api/media/<id>)`. The field takes a
// placeholder at once, so typing goes on while the upload runs; the placeholder becomes the
// picture, or goes away with an error, and the caret stays where the writer has it. A paste
// without pictures is the field's own. `update` must edit the field's text from its latest
// state rather than from what it rendered with: the upload ends after any number of keystrokes,
// and the `update` it calls is the one the paste saw. While `uploading`, saving would keep a
// placeholder, so callers hold it back.
// See specs/decisions/web/echo/20261113-paste-pictures-into-jottings-as-palace-media.md.
export function useImagePaste(
  field: RefObject<HTMLTextAreaElement | null>,
  update: (edit: (text: string) => string) => void,
) {
  const [pending, setPending] = useState(0);
  const [error, setError] = useState<string | null>(null);

  // Swaps a placeholder for what it became, keeping a focused field's caret on the same words.
  function settle(stand: string, become: string) {
    const element = field.current;
    const focused = element !== null && document.activeElement === element;
    const at = element?.value.indexOf(stand) ?? -1;
    const caret = focused
      ? [element.selectionStart, element.selectionEnd]
      : null;
    // The caret can move only once the new text is in the field.
    flushSync(() => update((text) => text.replace(stand, become)));
    if (element && caret && at >= 0) {
      const [start, end] = caret.map((position) =>
        shiftCaret(position, at, stand.length, become.length),
      );
      element.setSelectionRange(start, end);
    }
  }

  function onPaste(event: ClipboardEvent<HTMLTextAreaElement>) {
    const files = pastedImages(event.clipboardData);
    if (files.length === 0) return;
    event.preventDefault();
    const element = event.currentTarget;
    const named = files.map((file) => ({
      file,
      name: imageName(file),
      stand: "",
    }));
    for (const item of named) item.stand = placeholder(item.name, ++uploads);
    const inserted = named.map(({ stand }) => stand).join("\n");
    const { selectionStart: start, selectionEnd: end, value } = element;
    flushSync(() =>
      update(() => value.slice(0, start) + inserted + value.slice(end)),
    );
    element.setSelectionRange(start + inserted.length, start + inserted.length);
    setError(null);
    setPending((count) => count + named.length);
    for (const { file, name, stand } of named) {
      apiData(
        api.POST("/api/media", {
          body: { file },
          // The part keeps the picture's name and type; the browser owns the boundary.
          bodySerializer: ({ file }) => {
            const form = new FormData();
            form.set("file", file, name);
            return form;
          },
        }),
      )
        .then(
          ({ id }) => settle(stand, imageMarkdown(name, id)),
          (failure: unknown) => {
            settle(stand, "");
            setError(
              `图片上传失败：${failure instanceof Error ? failure.message : "请稍后重试。"}`,
            );
          },
        )
        .finally(() => setPending((count) => count - 1));
    }
  }

  return { onPaste, uploading: pending > 0, error };
}
