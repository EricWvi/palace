// The extension page: every web page the reader annotated or bound, and the settings. The token
// field is write only: the service worker never hands a stored token back.
import { h } from "./dom";
import { send } from "./protocol";

const list = document.getElementById("pages") as HTMLOListElement;
const listStatus = document.getElementById("pages-status")!;
const form = document.getElementById("settings") as HTMLFormElement;
const settingsStatus = document.getElementById("settings-status")!;
const server = form.elements.namedItem("server") as HTMLInputElement;
const token = form.elements.namedItem("token") as HTMLInputElement;

async function loadPages() {
  const reply = await send({ kind: "pages" });
  if (!reply.ok) {
    list.replaceChildren();
    listStatus.textContent = reply.message;
    return;
  }
  const items = reply.data.items;
  listStatus.textContent = items.length ? "" : "还没有标注过网页。";
  list.replaceChildren(
    ...items.map((page) =>
      h(
        "li",
        {},
        h(
          "a",
          { href: page.url, target: "_blank", rel: "noreferrer" },
          page.title,
        ),
        h(
          "span",
          { className: "meta" },
          [
            page.article_id === null
              ? `${page.annotation_count} 处标注`
              : page.annotation_count
                ? `已收进 Palace，${page.annotation_count} 处没对上`
                : "已收进 Palace",
            new Date(page.updated_at).toLocaleString("zh-CN"),
            page.url,
          ].join(" · "),
        ),
      ),
    ),
  );
}

async function loadSettings() {
  const reply = await send({ kind: "settings" });
  if (!reply.ok) return;
  server.value = reply.data.server;
  token.placeholder = reply.data.configured
    ? "已设置，填写新的以替换"
    : "palace_pat_…";
  token.required = !reply.data.configured;
}

form.addEventListener("submit", async (event) => {
  event.preventDefault();
  settingsStatus.textContent = "正在检查…";
  const reply = await send({
    kind: "save-settings",
    server: server.value,
    token: token.value,
  });
  settingsStatus.textContent = reply.ok ? "已保存。" : reply.message;
  if (!reply.ok) return;
  token.value = "";
  await Promise.all([loadSettings(), loadPages()]);
});

void loadSettings();
void loadPages();
