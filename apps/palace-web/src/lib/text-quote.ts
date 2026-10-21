// Finding a quote taken from one text in another text with the same words: a web page and the
// article Palace imported from it. The two agree on the words but not on the whitespace between
// blocks (Palace's annotation text joins blocks directly, a page's DOM text puts newlines and
// indentation between them), so both sides are compared with every whitespace character removed.
// The browser extension shares this module to draw an article's annotations back on the page.
// See specs/decisions/extension/0-web-page-annotations-bound-and-migrated-into-articles.md (D5, D6).

// What the extension keeps of a selection: the W3C text quote selector, plus where the quote
// started as a fraction of the page text.
export type TextQuote = {
  quote: string;
  prefix: string;
  suffix: string;
  position: number;
};

// A range of the original text, with the text it covers.
export type Located = { start: number; end: number; quote: string };

const space = /\s/u;

// The text without whitespace, and for each character kept, its index in the original text.
// Indexes are UTF-16 code units, like the annotation offsets; a surrogate pair stays two units.
export function squeeze(text: string): { text: string; origin: number[] } {
  let kept = "";
  const origin: number[] = [];
  for (let index = 0; index < text.length; index++) {
    const unit = text[index];
    if (space.test(unit)) continue;
    kept += unit;
    origin.push(index);
  }
  return { text: kept, origin };
}

// Every start of `needle` in `haystack`, overlapping ones included.
function occurrences(haystack: string, needle: string): number[] {
  const found: number[] = [];
  if (needle === "") return found;
  for (
    let at = haystack.indexOf(needle);
    at !== -1;
    at = haystack.indexOf(needle, at + 1)
  )
    found.push(at);
  return found;
}

// How many characters before `at` in `text` agree with the end of `prefix`, walking backwards.
function agreeBefore(text: string, at: number, prefix: string) {
  let n = 0;
  while (
    n < prefix.length &&
    n < at &&
    text[at - 1 - n] === prefix[prefix.length - 1 - n]
  )
    n++;
  return n;
}

// How many characters from `at` in `text` agree with the start of `suffix`.
function agreeAfter(text: string, at: number, suffix: string) {
  let n = 0;
  while (
    n < suffix.length &&
    at + n < text.length &&
    text[at + n] === suffix[n]
  )
    n++;
  return n;
}

// Maps a match in the squeezed text back to the original: from its first character to just
// after its last, so whitespace around the match stays outside and inside it stays as written.
function widen(
  text: string,
  origin: number[],
  at: number,
  length: number,
): Located {
  const start = origin[at];
  const end = origin[at + length - 1] + 1;
  return { start, end, quote: text.slice(start, end) };
}

// Finds where a web annotation's quote sits in `text`: among the exact matches of the squeezed
// quote, the one whose surroundings agree longest with the prefix and suffix, then the one whose
// start is nearest `position` as a fraction of `text`. No match is no location: a near miss
// could mark a similar but wrong sentence.
export function relocate(text: string, selector: TextQuote): Located | null {
  const squeezed = squeeze(text);
  const quote = squeeze(selector.quote).text;
  const prefix = squeeze(selector.prefix).text;
  const suffix = squeeze(selector.suffix).text;
  let best: { at: number; score: number; distance: number } | null = null;
  for (const at of occurrences(squeezed.text, quote)) {
    const score =
      agreeBefore(squeezed.text, at, prefix) +
      agreeAfter(squeezed.text, at + quote.length, suffix);
    const distance = Math.abs(
      squeezed.origin[at] / text.length - selector.position,
    );
    if (
      best === null ||
      score > best.score ||
      (score === best.score && distance < best.distance)
    )
      best = { at, score, distance };
  }
  return best && widen(text, squeezed.origin, best.at, quote.length);
}

// Finds an article annotation's quote in a page's text for the read-only display: the match
// whose start is nearest `position`, a fraction of the page text. Without context the position
// is all there is to tell repeated quotes apart.
export function findQuote(
  text: string,
  quote: string,
  position: number,
): Located | null {
  return relocate(text, { quote, prefix: "", suffix: "", position });
}
