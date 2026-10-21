// Annotating is off on every page until the reader turns it on for one tab in the popup (see
// specs/decisions/extension/20261021-annotating-off-until-turned-on-per-tab.md). The switch is
// kept per tab in `chrome.storage.session`, so it lasts while the tab navigates and goes with the
// tab or the browser session. Session storage is closed to content scripts, so a page asks the
// service worker about its own tab.

export type Switch = "on" | "off";

// A content script's question about its own tab; the service worker answers with a `Switch`.
export type SwitchQuery = { kind: "switch" };

// The popup's order to the content script of a tab, which answers once it has obeyed.
export type SwitchOrder = { kind: "turn"; to: Switch };

const key = (tab: number) => `annotating-${tab}`;

export async function readSwitch(tab: number): Promise<Switch> {
  const stored = await chrome.storage.session.get(key(tab));
  return stored[key(tab)] === "on" ? "on" : "off";
}

// Off is the absence of an entry, so a closed tab leaves nothing behind once its entry goes.
export async function writeSwitch(tab: number, to: Switch) {
  if (to === "on") await chrome.storage.session.set({ [key(tab)]: "on" });
  else await chrome.storage.session.remove(key(tab));
}

// Whether `url` is a page of Palace itself. Palace draws its own annotations and bar there, and
// the extension's would sit on top of them, so annotating cannot be turned on there.
export function onPalace(url: string, server: string): boolean {
  try {
    return new URL(url).origin === new URL(server).origin;
  } catch {
    return false;
  }
}
