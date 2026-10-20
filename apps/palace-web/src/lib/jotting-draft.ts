// A `#tag` starts the text or follows whitespace, so `C#` and `a.html#x` stay text; `# 标题`
// is a Markdown heading because a space follows the `#`. Spaces after a tag go with it; the one
// before it is only looked at, so `#a #b` is two tags.
const TAG = /(?<=^|[ \t])#([^\s#]+)[ \t]*/g;
const FENCE = /^\s*(```|~~~)/;

// Splits a draft into tags and body the flomo way: `#tag` becomes a tag and leaves the text.
// Only what the tags leave behind is cleaned up — a line that held nothing but tags, and the
// space a tag ended a line with — so indentation and code reach the server exactly as typed.
// Fenced code is never searched for tags.
export function parseDraft(text: string): { tags: string[]; body: string } {
  const tags: string[] = [];
  const lines: string[] = [];
  let fenced = false;
  for (const line of text.split("\n")) {
    if (FENCE.test(line)) fenced = !fenced;
    if (fenced || FENCE.test(line) || !line.match(TAG)) {
      lines.push(line);
      continue;
    }
    const rest = line
      .replace(TAG, (_, tag: string) => {
        if (!tags.includes(tag)) tags.push(tag);
        return "";
      })
      .trimEnd();
    if (rest.trim()) lines.push(rest);
  }
  // Blank lines around the text go, but not the first line's indentation.
  const body = lines
    .join("\n")
    .replace(/^(?:[ \t]*\n)+/, "")
    .trimEnd();
  return { tags, body };
}

// The `#fragment` being typed right before the caret, or null when the caret is not in one.
export function tagFragment(text: string, caret: number): string | null {
  return text.slice(0, caret).match(/(?:^|\s)#([^\s#]*)$/)?.[1] ?? null;
}

// Known tags containing the fragment, in the given order (most used first), at most six; the
// fragment itself is already complete and not offered.
export function suggestTags(known: string[], fragment: string): string[] {
  return known
    .filter((tag) => tag.includes(fragment) && tag !== fragment)
    .slice(0, 6);
}

// Replaces the fragment before the caret with the whole tag and a space, returning the new text
// and where the caret goes.
export function completeTag(
  text: string,
  caret: number,
  tag: string,
): { text: string; caret: number } {
  const before = text.slice(0, caret).replace(/#([^\s#]*)$/, `#${tag} `);
  return { text: before + text.slice(caret), caret: before.length };
}
