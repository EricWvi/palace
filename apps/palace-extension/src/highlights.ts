// Draws annotations on a web page with the CSS Custom Highlight API: ranges are styled without
// touching the page's DOM, so site scripts keep the tree they built and site CSS cannot reach
// the marks (D6). One registry entry per look, as Palace draws them.
import tokens from "../../../design/tokens.css?raw";
import type { AnnotationStyle } from "./protocol";

// How a mark looks: one of the six styles, or a note with no style of its own.
export type Look = AnnotationStyle | "plain";

const looks: Look[] = [
  "yellow",
  "green",
  "blue",
  "rose",
  "underline",
  "wavy",
  "plain",
];

// A design token of the light theme, following `var()` references. Pages bring their own
// colours, so marks keep the light grounds and set dark text on them, readable on any site.
export function lightToken(name: string): string {
  const block = /\[data-theme="light"\]\s*\{([^}]*)\}/.exec(tokens)?.[1] ?? "";
  const value = new RegExp(`--${name}:\\s*([^;]+);`).exec(block)?.[1].trim();
  if (value === undefined) throw new Error(`no light token --${name}`);
  const reference = /^var\(--([\w-]+)\)$/.exec(value);
  return reference ? lightToken(reference[1]) : value;
}

const name = (look: Look) => `palace-${look}`;

function rule(look: Look): string {
  const ink = lightToken("text");
  switch (look) {
    case "underline":
      return `text-decoration: underline 2px ${lightToken("mark-line")}; text-underline-offset: 5px;`;
    case "wavy":
      return `text-decoration: underline wavy 1.5px ${lightToken("mark-wave")}; text-underline-offset: 4px;`;
    case "plain":
      return `background-color: color-mix(in srgb, ${lightToken("accent")} 12%, transparent); text-decoration: underline 1.5px color-mix(in srgb, ${lightToken("accent")} 45%, transparent);`;
    case "yellow":
    case "green":
    case "blue":
    case "rose":
      return `background-color: ${lightToken(`mark-${look}`)}; color: ${ink};`;
  }
}

let styled = false;

// Adds the highlight rules to the page once. An adopted sheet lives outside the DOM tree.
function installStyles() {
  if (styled) return;
  styled = true;
  const sheet = new CSSStyleSheet();
  sheet.replaceSync(
    looks
      .map((look) => `::highlight(${name(look)}) { ${rule(look)} }`)
      .join("\n"),
  );
  document.adoptedStyleSheets = [...document.adoptedStyleSheets, sheet];
}

// Replaces every mark on the page with `marks`.
export function paint(marks: { range: Range; look: Look }[]) {
  installStyles();
  for (const look of looks) {
    const ranges = marks
      .filter((mark) => mark.look === look)
      .map((mark) => mark.range);
    if (ranges.length) CSS.highlights.set(name(look), new Highlight(...ranges));
    else CSS.highlights.delete(name(look));
  }
}
