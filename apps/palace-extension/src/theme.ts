// The toolbar icon follows the browser's light or dark appearance. The manifest holds one static
// icon and the service worker cannot evaluate media queries, so every page the extension runs in
// reports the appearance it sees, now and whenever it changes.

export type ThemeReport = { kind: "theme"; dark: boolean };

export function reportTheme() {
  const query = matchMedia("(prefers-color-scheme: dark)");
  const report = () =>
    // An extension reloaded under an open page no longer answers; the page has nothing to fix.
    chrome.runtime
      .sendMessage({ kind: "theme", dark: query.matches } satisfies ThemeReport)
      .catch(() => {});
  void report();
  query.addEventListener("change", () => void report());
}
