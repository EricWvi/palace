// The toolbar popup: turning annotating on or off for the current tab, what the page is to
// Palace, and binding it to an imported article. Binding opens the article in Palace, where the
// page's annotations migrate (D3, D4). A tab left off asks Palace nothing.
import { h } from "./dom";
import {
  pageAddress,
  send,
  type ArticlePage,
  type PageState,
  type Reply,
} from "./protocol";
import { onPalace, readSwitch, writeSwitch, type SwitchOrder } from "./switch";
import { reportTheme } from "./theme";

const main = document.getElementById("popup")!;

type Tab = { id: number; url: string; title: string };

function show(...children: Node[]) {
  main.replaceChildren(...children);
}

// A view's actions, then the way to the extension page, under every view.
function footer(...actions: Node[]) {
  return h(
    "p",
    { className: "row" },
    ...actions,
    h(
      "button",
      {
        className: "link",
        onclick: () => void chrome.runtime.openOptionsPage(),
      },
      "标注过的网页与设置",
    ),
  );
}

// Every view of a tab turned on offers to turn it off again.
function turnOffButton(tab: Tab) {
  return h(
    "button",
    { className: "link", onclick: () => void turnOff(tab) },
    "关闭标注",
  );
}

function problem(tab: Tab, reply: Extract<Reply<unknown>, { ok: false }>) {
  show(h("p", {}, reply.message), footer(turnOffButton(tab)));
}

// Tells the tab's content script; a tab opened before the extension was loaded has none.
function order(tab: Tab, to: SwitchOrder["to"]): Promise<boolean> {
  return chrome.tabs
    .sendMessage(tab.id, { kind: "turn", to } satisfies SwitchOrder)
    .then(
      () => true,
      () => false,
    );
}

function off(tab: Tab) {
  show(
    h("p", {}, "这个标签页没有开启标注。"),
    h(
      "p",
      { className: "quiet" },
      "开启后，插件在这个标签页里显示和保存标注；换网页、刷新都保持开启，直到关闭标注或关掉标签页。",
    ),
    footer(
      h(
        "button",
        { className: "link", onclick: () => void turnOn(tab) },
        "在这个标签页开启标注",
      ),
    ),
  );
}

async function turnOn(tab: Tab) {
  await writeSwitch(tab.id, "on");
  const reached = await order(tab, "on");
  await view(tab);
  if (!reached)
    main.prepend(h("p", { className: "quiet" }, "刷新这个网页后开始标注。"));
}

async function turnOff(tab: Tab) {
  await writeSwitch(tab.id, "off");
  await order(tab, "off");
  // The badge spoke for the page's annotations, which the tab no longer shows.
  await chrome.action.setBadgeText({ tabId: tab.id, text: "" });
  await chrome.action.setTitle({ tabId: tab.id, title: "Palace" });
  off(tab);
}

// What a tab turned on shows: the page as Palace knows it.
async function view(tab: Tab) {
  const url = pageAddress(tab.url);
  const reply = await send({ kind: "page", url });
  if (!reply.ok) return problem(tab, reply);
  render(tab, url, reply.data);
}

async function openArticle(tab: Tab, state: PageState) {
  if (state.kind !== "bound") return;
  // The page turns read only at once; a tab without the content script has nothing to refresh.
  await chrome.tabs.sendMessage(tab.id, { kind: "refresh" }).catch(() => {});
  await chrome.tabs.create({ url: state.article.link });
  window.close();
}

// The candidate list: articles nearest this address first, narrowed by a search.
function choose(tab: Tab, url: string) {
  const search = h("input", {
    type: "search",
    placeholder: "搜索文章",
    ariaLabel: "搜索文章",
  });
  const list = h("ol", { className: "candidates" });
  const status = h("p", { className: "quiet" });
  let query = 0;

  async function load(cursor: string | null) {
    const asked = ++query;
    const reply = await send({
      kind: "candidates",
      near: url,
      q: search.value.trim(),
      cursor,
    });
    if (asked !== query) return;
    if (!reply.ok) return problem(tab, reply);
    if (cursor === null) list.replaceChildren();
    list.querySelector(".more")?.remove();
    append(reply.data);
    status.textContent = list.children.length === 0 ? "没有找到文章。" : "";
  }

  function append(page: ArticlePage) {
    for (const item of page.items)
      list.append(
        h(
          "li",
          {},
          h(
            "button",
            {
              onclick: async () => {
                const reply = await send({
                  kind: "bind",
                  url,
                  title: tab.title,
                  article_id: item.id,
                });
                if (!reply.ok) return problem(tab, reply);
                await openArticle(tab, reply.data);
              },
            },
            item.title,
            h(
              "span",
              { className: "meta" },
              `${item.author ?? item.site} · ${item.url}`,
            ),
          ),
        ),
      );
    if (page.next_cursor)
      list.append(
        h(
          "li",
          { className: "more" },
          h("button", { onclick: () => void load(page.next_cursor) }, "更多"),
        ),
      );
  }

  let typing = 0;
  search.addEventListener("input", () => {
    clearTimeout(typing);
    typing = window.setTimeout(() => void load(null), 250);
  });
  show(search, list, status);
  search.focus();
  void load(null);
}

function render(tab: Tab, url: string, state: PageState) {
  if (state.kind === "bound") {
    show(
      h("p", { className: "quiet" }, "已收进 Palace"),
      h(
        "p",
        { className: "title" },
        h(
          "a",
          {
            href: state.article.link,
            onclick: (event: Event) => {
              event.preventDefault();
              void chrome.tabs.create({ url: state.article.link });
            },
          },
          state.article.title,
        ),
      ),
      footer(
        h(
          "button",
          { className: "link", onclick: () => choose(tab, url) },
          "改绑到另一篇",
        ),
        turnOffButton(tab),
      ),
    );
    return;
  }
  const count = state.annotations.length;
  show(
    h("p", {}, count ? `这个网页有 ${count} 处标注。` : "这个网页还没有标注。"),
    footer(
      h(
        "button",
        { className: "link", onclick: () => choose(tab, url) },
        "收进 Palace",
      ),
      turnOffButton(tab),
    ),
  );
}

async function start() {
  const [active] = await chrome.tabs.query({
    active: true,
    currentWindow: true,
  });
  if (!active?.id || !active.url || !/^https?:/.test(active.url)) {
    show(h("p", { className: "quiet" }, "这个页面不能标注。"), footer());
    return;
  }
  const tab = { id: active.id, url: active.url, title: active.title ?? "" };
  // Reading the settings stays inside the extension; it asks Palace nothing.
  const settings = await send({ kind: "settings" });
  if (
    settings.ok &&
    settings.data.configured &&
    onPalace(tab.url, settings.data.server)
  ) {
    show(
      h(
        "p",
        { className: "quiet" },
        "这是 Palace 本身，请用 Palace 自己的标注。",
      ),
      footer(),
    );
    return;
  }
  if ((await readSwitch(tab.id)) === "off") return off(tab);
  await view(tab);
}

reportTheme();
void start();
