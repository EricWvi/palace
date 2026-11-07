# 总批

批注属于一段文字，总批属于整个文档：一篇文章、一条对话 Path、一个原网页各有一块全局文本，用固定在视口里的卡片书写和阅读。决策见[总批 ADR](../../../specs/decisions/web/stars/20261107-overall-notes-on-articles-paths-and-web-pages.md)，交互原型见 [`design/3.4-stars-overall-note.html`](../../../design/3.4-stars-overall-note.html)（变体 A）。

笔记没有总批。总批不随文章导出，也不进入搜索、时间线和回顾。

## 规则

- 内容是 Markdown，不超过 64 KiB。清空等于删除。
- 对话的总批属于 Path，切换分支时卡片跟着换，删除分支时一起删除。
- 删除文章、对话或网页时，总批在同一事务里删除。
- 保存要带读到的版本。版本不符时什么都不写，返回 409 `overall_note_changed`，这样两个标签页同时开着也不会互相抹掉。版本来自数据库序列，不会重复；没有总批时版本是 `0`。

## Palace 阅读页

卡片不进入文档流，标题和 meta 行保持原样。

- 没有总批时页面上什么都没有，按 `n` 打开。
- 有总批时，窗口至少 1180px 宽的话，批注栏底部显示一行「总批」和第一行文字，页面像有批注时一样加宽；更窄时右下角只有「总批」二字的小标签。
- 点击它展开阅读，显示渲染后的 Markdown；再点击正文进入编辑。`n` 直接进入编辑。展开的卡片高度随内容增长，上限 40vh。
- 编辑时停止输入约 800ms 自动保存一次，离开、`Esc` 或 `⌘/Ctrl+Enter` 时再保存一次并收起。保存失败时卡片保持展开，保留输入并提示。
- 输入框里、选区小条或弹窗显示时，`n` 不起作用。

## 插件

标签页开启标注后，右下角一直有这张卡片，空的时候也显示一行「总批」。卡片在 closed shadow root 里，只有纯文本编辑。

- 快捷键默认 `Alt+Shift+N`，可以在 `chrome://extensions/shortcuts` 修改。在没有开启的标签页上按它，等于开启这个标签页并打开卡片。Palace 自己的页面不起作用。
- 自动保存的规则与 Palace 相同。没保存成功的文字按网址存在本机的 `chrome.storage.local`，连同它所基于的版本，保存成功后删除；再打开这个网页时草稿优先显示，并提示还没保存。
- 网页绑定文章时，它的总批在绑定的事务里并入文章：文章没有总批就直接移入，已有时追加在后面，用一行 `---` 隔开。合并后超过 64 KiB 时绑定失败，返回 400，什么都不改。
- 绑定之后，这张卡片直接读写文章的总批。改绑时已并入的内容留在原来的文章。
- 只有总批、没有标注的网页也会保留记录；总批清空后如果也没有标注，网页记录一起删除。

## 接口

| 方法与路径 | 作用 |
| --- | --- |
| `GET /api/articles/{id}/overall-note` | 返回 `{ text, version }`，没有时 `text` 为 `null`、`version` 为 `0` |
| `PUT /api/articles/{id}/overall-note` | `{ text, version }`，空白文本等于删除 |
| `GET`、`PUT /api/conversations/{id}/paths/{path_id}/overall-note` | 同上 |
| `PUT /api/web-pages/overall-note` | `{ url, title, text, version }`，网页不存在时一并创建；已绑定返回 409 `web_page_bound` |

`GET /api/web-pages/lookup` 的响应带 `overall_note`，已绑定时是文章的总批。

## 工程入口

- 数据库：`crates/db/migrations/0028_overall_notes.sql`、`crates/db/src/overall_note.rs`；校验：`crates/domain/src/overall_note.rs`；接口：`crates/backend/src/http/overall_notes.rs`。
- 前端：`apps/palace-web/src/components/overall-note.tsx`、`annotations.css`。
- 插件：`apps/palace-extension/src/note-pad.ts`（卡片）、`src/palace.ts`（草稿与保存）。
