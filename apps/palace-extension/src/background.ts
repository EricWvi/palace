// The service worker: wires the browser's storage, fetch, messages and toolbar badge to the
// request handler in palace.ts.
import { createHandler, type Settings, type SettingsStore } from "./palace";
import type { PageState, Reply, Request } from "./protocol";
import {
  onPalace,
  readSwitch,
  writeSwitch,
  type Switch,
  type SwitchQuery,
} from "./switch";
import type { ThemeReport } from "./theme";

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

// What a content script's tab is set to. Palace's own pages stay off even in a tab turned on,
// since the tab may have navigated there.
async function switchOf(sender: chrome.runtime.MessageSender): Promise<Switch> {
  const tab = sender.tab?.id;
  if (tab === undefined || sender.url === undefined) return "off";
  const settings = await store.read();
  if (settings !== null && onPalace(sender.url, settings.server)) return "off";
  return readSwitch(tab);
}

chrome.tabs.onRemoved.addListener((tab) => void writeSwitch(tab, "off"));

chrome.runtime.onMessage.addListener(
  (request: Request | ThemeReport | SwitchQuery, sender, respond) => {
    // The pale quill reads on a dark toolbar, the ink one on a light toolbar.
    if (request.kind === "theme") {
      void chrome.action.setIcon({
        path: request.dark ? "icon-dark.png" : "icon.png",
      });
      return false;
    }
    if (request.kind === "switch") {
      void switchOf(sender).then(respond);
      return true;
    }
    // Extension pages share the extension's origin; content scripts report the page's.
    const own = sender.url?.startsWith(chrome.runtime.getURL("")) ?? false;
    void handle(request, own ? "extension" : "content").then((reply) => {
      if (request.kind === "page" && !own && sender.tab?.id !== undefined)
        badge(sender.tab.id, reply);
      respond(reply);
    });
    // The reply is sent asynchronously.
    return true;
  },
);
