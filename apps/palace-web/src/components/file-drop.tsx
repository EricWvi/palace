import { useState } from "react";

// A file field drawn as a quiet box: click it to browse, or drop a file onto it. The real
// input stays inside, visually hidden, so labels, keyboard focus and form semantics still
// come from the browser.
export function FileDrop({
  id,
  accept,
  file,
  onFile,
}: {
  id: string;
  accept: string;
  file: File | null;
  onFile: (file: File | null) => void;
}) {
  const [over, setOver] = useState(false);
  return (
    <label
      htmlFor={id}
      className="file-drop"
      data-over={over || undefined}
      data-filled={file !== null || undefined}
      onDragOver={(event) => {
        // Without this the browser opens a dropped file instead of handing it to the page.
        event.preventDefault();
        setOver(true);
      }}
      onDragLeave={() => setOver(false)}
      onDrop={(event) => {
        event.preventDefault();
        setOver(false);
        const dropped = event.dataTransfer.files[0];
        if (dropped) onFile(dropped);
      }}
    >
      <input
        id={id}
        type="file"
        accept={accept}
        className="sr-only"
        onChange={(event) => onFile(event.target.files?.[0] ?? null)}
      />
      <span className="file-drop-main">{file?.name ?? "选择 JSON 文件"}</span>
      <span className="file-drop-sub">
        {file ? "点击更换，或将其他文件拖到这里" : "或将文件拖到这里"}
      </span>
    </label>
  );
}
