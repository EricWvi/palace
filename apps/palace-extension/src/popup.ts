// The toolbar popup: what the current page is to Palace, and binding it to an imported article.
// Binding opens the article in Palace, where the page's annotations migrate (D3, D4).
import { h } from "./dom";
import {
  pageAddress,
  send,
  type ArticlePage,
  type PageState,
  type Reply,
} from "./protocol";

const main = document.getElementById("popup")!;

type Tab = { id: number; url: string; title: string };

function show(...children: Node[]) {
  main.replaceChildren(...children);
}

// The way to the extension page, under every view.
function footer() {
  return h(
    "p",
    { className: "row" },
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

function problem(reply: Extract<Reply<unknown>, { ok: false }>) {
  show(h("p", {}, reply.message), footer());
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
    if (!reply.ok) return problem(reply);
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
                if (!reply.ok) return problem(reply);
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
      h(
        "p",
        { className: "row" },
        h(
          "button",
          { className: "link", onclick: () => choose(tab, url) },
          "改绑到另一篇",
        ),
        h(
          "button",
          {
            className: "link",
            onclick: () => void chrome.runtime.openOptionsPage(),
          },
          "标注过的网页与设置",
        ),
      ),
    );
    return;
  }
  const count = state.annotations.length;
  show(
    h("p", {}, count ? `这个网页有 ${count} 处标注。` : "这个网页还没有标注。"),
    h(
      "p",
      { className: "row" },
      h(
        "button",
        { className: "link", onclick: () => choose(tab, url) },
        "收进 Palace",
      ),
      h(
        "button",
        {
          className: "link",
          onclick: () => void chrome.runtime.openOptionsPage(),
        },
        "标注过的网页与设置",
      ),
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
  const url = pageAddress(tab.url);
  const reply = await send({ kind: "page", url });
  if (!reply.ok) return problem(reply);
  render(tab, url, reply.data);
}

void start();
