// A fenced block or inline code span, kept as it is; then a display formula alone on its line,
// one inside a sentence, and an inline formula. Only exactly two backslashes delimit a formula:
// ChatGPT copies `\(…\)` and `\[…\]` Markdown-escaped, while one backslash only escapes a bracket.
const MATH_OR_CODE =
  /(^[ \t>]*(`{3,}|~{3,})[^\n]*\n[\s\S]*?^[ \t>]*\2[ \t]*$|(`+)[^`]+?\3)|^([ \t>]*)\\\\\[([\s\S]+?)\\\\\][ \t]*$|(?<!\\)\\\\\[([\s\S]+?)\\\\\]|(?<!\\)\\\\\(([\s\S]+?)\\\\\)/gm;

// Formula bodies are Markdown-escaped too (`\_`, `\<`, a TeX line break as `\\\\`). CommonMark
// escapes only ASCII punctuation, so undoing just those leaves commands such as `\bigvee` alone.
const MARKDOWN_ESCAPE = /\\([!-/:-@[-`{-~])/g;

function unescapeTex(tex: string): string {
  return tex.trim().replace(MARKDOWN_ESCAPE, "$1");
}

// Turns ChatGPT's copied `\\(…\\)` and `\\[…\\]` formulas into the `$…$` and `$$` blocks the
// reader typesets, as the Automa export does. The older dollar syntax has neither delimiter and
// passes through unchanged, so a reply copied in either format reads the same. A display formula
// alone on its line becomes a multi-line `$$` block that keeps its list indent or quote prefix.
export function dollarMath(markdown: string): string {
  return markdown.replace(
    MATH_OR_CODE,
    (
      match,
      code?: string,
      _fence?: string,
      _ticks?: string,
      prefix?: string,
      block?: string,
      display?: string,
      inline?: string,
    ) => {
      if (code !== undefined) return match;
      if (block !== undefined)
        return `${prefix}$$\n${prefix}${unescapeTex(block)}\n${prefix}$$`;
      if (display !== undefined) return `$$${unescapeTex(display)}$$`;
      return `$${unescapeTex(inline!)}$`;
    },
  );
}
