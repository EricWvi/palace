import type { HtmlTagDescriptor, Plugin } from "vite";

// The face the first screen's Latin text is set in: the day heading and the card titles. Other
// weights, italics and subsets are not on that screen and would only compete for early bandwidth.
const FIRST_PAINT_FONT = /^assets\/eb-garamond-latin-400-normal-[\w-]+\.woff2$/;

/**
 * Builds the preload link for the first-paint font out of the emitted file names.
 *
 * The font is otherwise requested only after the script has run and rendered text in it, so the
 * page first paints in a fallback face and visibly swaps. Exactly one match is required: a
 * renamed file after a fontsource upgrade must fail the build rather than silently drop the link.
 */
export function firstPaintFontPreload(
  fileNames: readonly string[],
): HtmlTagDescriptor {
  const matches = fileNames.filter((name) => FIRST_PAINT_FONT.test(name));
  if (matches.length !== 1) {
    throw new Error(
      `expected one file matching ${FIRST_PAINT_FONT}, found ${matches.length}: ${matches.join(", ")}`,
    );
  }
  return {
    tag: "link",
    // Fonts are always fetched in CORS mode; without `crossorigin` the preload would not be
    // reused and the font would download twice.
    attrs: {
      rel: "preload",
      as: "font",
      type: "font/woff2",
      href: `/${matches[0]}`,
      crossorigin: true,
    },
    injectTo: "head",
  };
}

/** Adds the first-paint font preload to the built `index.html`; the dev server goes without. */
export function preloadFirstPaintFont(): Plugin {
  return {
    name: "palace:preload-first-paint-font",
    apply: "build",
    transformIndexHtml(_html, { bundle }) {
      return [firstPaintFontPreload(Object.keys(bundle ?? {}))];
    },
  };
}
