// Draws annotations on a web page with the CSS Custom Highlight API: ranges are styled without
// touching the page's DOM, so site scripts keep the tree they built and site CSS cannot reach
// the marks (D6). One registry entry per look and ground, as Palace draws them.
import tokens from "../../../design/tokens.css?raw";
import type { AnnotationStyle } from "./protocol";

// How a mark looks: one of the six styles, or a note with no style of its own.
export type Look = AnnotationStyle | "plain";

// Which of Palace's two palettes suits the ground under a mark.
export type Ground = "light" | "dark";

const looks: Look[] = [
  "yellow",
  "green",
  "blue",
  "rose",
  "underline",
  "wavy",
  "plain",
];

const grounds: Ground[] = ["light", "dark"];

// A design token of one theme, following `var()` references. A theme's tokens are spread over
// several blocks whose selector ends with that theme, so every such block is searched.
export function themeToken(ground: Ground, name: string): string {
  const blocks = tokens.matchAll(
    new RegExp(`\\[data-theme="${ground}"\\]\\s*\\{([^}]*)\\}`, "g"),
  );
  for (const [, block] of blocks) {
    const value = new RegExp(`--${name}:\\s*([^;]+);`).exec(block)?.[1].trim();
    if (value === undefined) continue;
    const reference = /^var\(--([\w-]+)\)$/.exec(value);
    return reference ? themeToken(ground, reference[1]) : value;
  }
  throw new Error(`no ${ground} token --${name}`);
}

// The ground a text colour is written on: light ink means a dark page. The ink is read rather
// than the background because a site's ground may be an image, a gradient or the canvas, while
// its text colour is always computed; and a site may be dark or light whatever the browser's
// own appearance says.
export function groundOf(ink: string): Ground {
  const channels =
    /^rgba?\((\d+(?:\.\d+)?),\s*(\d+(?:\.\d+)?),\s*(\d+(?:\.\d+)?)/.exec(ink);
  if (!channels)
    return matchMedia("(prefers-color-scheme: dark)").matches
      ? "dark"
      : "light";
  const [r, g, b] = channels.slice(1).map(Number);
  // Rec. 709 luma on the gamma-encoded channels is enough to tell light ink from dark.
  return 0.2126 * r + 0.7152 * g + 0.0722 * b > 128 ? "dark" : "light";
}

const name = (look: Look, ground: Ground) => `palace-${look}-${ground}`;

// The marks set their own ink on their grounds, readable on any site, as the prototype draws them.
function rule(look: Look, ground: Ground): string {
  const token = (name: string) => themeToken(ground, name);
  switch (look) {
    case "underline":
      return `text-decoration: underline 2px ${token("mark-line")}; text-decoration-skip-ink: none; text-underline-offset: 5px;`;
    case "wavy":
      return `text-decoration: underline wavy 1.5px ${token("mark-wave")}; text-decoration-skip-ink: none; text-underline-offset: 4px;`;
    case "plain":
      return `background-color: color-mix(in srgb, ${token("accent")} 8%, transparent); text-decoration: underline 1.5px color-mix(in srgb, ${token("accent")} 45%, transparent);`;
    case "yellow":
    case "green":
    case "blue":
    case "rose":
      return `background-color: ${token(`mark-${look}`)}; color: ${token("text")};`;
  }
}

let styled = false;

// Adds the highlight rules to the page once. An adopted sheet lives outside the DOM tree.
function installStyles() {
  if (styled) return;
  styled = true;
  const sheet = new CSSStyleSheet();
  sheet.replaceSync(
    grounds
      .flatMap((ground) =>
        looks.map(
          (look) =>
            `::highlight(${name(look, ground)}) { ${rule(look, ground)} }`,
        ),
      )
      .join("\n"),
  );
  document.adoptedStyleSheets = [...document.adoptedStyleSheets, sheet];
}

// Replaces every mark on the page with `marks`.
export function paint(marks: { range: Range; look: Look }[]) {
  installStyles();
  // Each mark takes the ground under the text where it starts, so a dark sidebar on a light page
  // gets the dark palette.
  const placed = marks.map((mark) => {
    const node = mark.range.startContainer;
    const element = node instanceof Element ? node : node.parentElement;
    return {
      ...mark,
      ground: groundOf(element ? getComputedStyle(element).color : ""),
    };
  });
  for (const ground of grounds)
    for (const look of looks) {
      const ranges = placed
        .filter((mark) => mark.look === look && mark.ground === ground)
        .map((mark) => mark.range);
      if (ranges.length)
        CSS.highlights.set(name(look, ground), new Highlight(...ranges));
      else CSS.highlights.delete(name(look, ground));
    }
}
