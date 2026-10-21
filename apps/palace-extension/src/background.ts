// The service worker: wires the browser's storage, fetch, messages and toolbar badge to the
// request handler in palace.ts.
import { createHandler, type Settings, type SettingsStore } from "./palace";
import type { PageState, Reply, Request } from "./protocol";

const key = "settings";

const store: SettingsStore = {
  async read() {
    const stored = await chrome.storage.local.get(key);
    return (stored[key] as Settings | undefined) ?? null;
  },
  async write(settings) {
    await chrome.storage.local.set({ [key]: settings });
  },
};

const handle = createHandler(store, (input, init) => fetch(input, init));

// The toolbar icon says what the extension can do on a tab: nothing until it reaches Palace,
// read only once the page is bound.
function badge(tab: number, reply: Reply<unknown>) {
  let text = "";
  let title = "Palace";
  if (!reply.ok) {
    text = "!";
    title = reply.problem === "unauthorized" ? "Access Token 无效" : "未连接";
  } else if ((reply.data as PageState).kind === "bound") {
    text = "✓";
    title = "已收进 Palace";
  }
  void chrome.action.setBadgeText({ tabId: tab, text });
  void chrome.action.setBadgeBackgroundColor({ tabId: tab, color: "#857154" });
  void chrome.action.setTitle({ tabId: tab, title });
}

chrome.runtime.onMessage.addListener((request: Request, sender, respond) => {
  // Extension pages share the extension's origin; content scripts report the page's.
  const own = sender.url?.startsWith(chrome.runtime.getURL("")) ?? false;
  void handle(request, own ? "extension" : "content").then((reply) => {
    if (request.kind === "page" && !own && sender.tab?.id !== undefined)
      badge(sender.tab.id, reply);
    respond(reply);
  });
  // The reply is sent asynchronously.
  return true;
});
