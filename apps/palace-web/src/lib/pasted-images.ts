// The pure half of pasting pictures into a jotting: which pasted files are pictures, what stands
// in the text while one uploads, what it becomes, and where the caret goes when it does.

// Image files on a paste. A screenshot arrives as one file named `image.png`; a picture copied
// in the file manager arrives as that file with its own name. Text, HTML and other files are
// left to the field's ordinary paste.
export function pastedImages(data: DataTransfer): File[] {
  return [...data.files].filter((file) => file.type.startsWith("image/"));
}

// The name a picture is written under: its own, or `image.<subtype>` when the clipboard gave
// none.
export function imageName(file: File) {
  return file.name || `image.${file.type.slice("image/".length) || "png"}`;
}

// Brackets and backslashes would end or escape the alt text early.
function altText(name: string) {
  return name.replace(/[\\[\]]/g, (char) => `\\${char}`);
}

// What a picture uploading stands as. The address names the upload, so two pictures of the same
// name are told apart; it is not a media address, so the zoomed card never loads it.
export function placeholder(name: string, upload: number) {
  return `![上传中 ${altText(name)}…](uploading-${upload})`;
}

// A stored picture as the zoomed card loads it.
export function imageMarkdown(name: string, id: string) {
  return `![${altText(name)}](/api/media/${id})`;
}

// Where a caret at `position` goes when `from` characters at `at` become `to` characters: after
// the change it moves by the difference, inside it to the replacement's end, before it stays.
export function shiftCaret(
  position: number,
  at: number,
  from: number,
  to: number,
) {
  if (position <= at) return position;
  if (position >= at + from) return position + to - from;
  return at + to;
}
