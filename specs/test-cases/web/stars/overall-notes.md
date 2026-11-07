# 总批核心测试用例

当前决策：[总批](../../../decisions/web/stars/20261107-overall-notes-on-articles-paths-and-web-pages.md)。

## An overall note must belong to exactly one document and go with it

风险：一个文档出现两块总批；分支切换后读到别的分支的总批；其他 Owner 能读写；删除文档后总批留在库里；清空后留下空白行。前置：真实 PostgreSQL；两个 Owner，一篇文章，一个有两条 Path 的对话。触发：分别读写文章和两条 Path 的总批；用另一 Owner 读写；提交空白文本；删除 Path、对话和文章。

必须成立：两条 Path 的总批互不影响；没有总批时读到 `text: null`、`version: 0`；跨 Owner 与不存在都返回 404；空白文本等于删除，行不再存在；删除文档后总批在同一事务中消失，删除一条 Path 不影响另一条。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 归属、Owner 隔离、清空与级联删除（真实 PostgreSQL） | Covered | `crates/db/tests/postgres/overall_notes.rs::overall_notes_belong_to_one_document_and_go_with_it` |
| 超过 64 KiB 被拒绝，空白视为清空（单元测试） | Covered | `crates/domain/src/overall_note.rs::text_is_bounded_and_blank_means_none` |
| 接口形状、状态码与 OpenAPI 合同一致（真实 Router） | Covered | `crates/backend/tests/http.rs::authenticated_http_imports_preserve_scope_and_file_parity`（经 `tests/http/overall_notes.rs`） |

决策依据：D1、D3、D4，不变量 1、2、3。

## A stale write must never replace an overall note

风险：两个标签页同时写时，后写入者静默抹掉前者的整段文字；清空再重写后版本号回到旧值，过期页面绕过检查；两个首次写入都成功。前置：真实 PostgreSQL；一块总批。触发：用旧版本写入；清空后重写，再用清空前的版本写入；两个请求用 barrier 同时以版本 0 首次写入。

必须成立：版本不符返回 409 `overall_note_changed`，内容不变；清空重写后旧版本仍然被拒绝；并发首次写入恰好一个成功。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 旧版本被拒绝，版本不重复使用（真实 PostgreSQL） | Covered | `crates/db/tests/postgres/overall_notes.rs::a_stale_version_never_replaces_an_overall_note` |
| 并发首次写入只有一个成功（真实 PostgreSQL） | Covered | `crates/db/tests/postgres/overall_notes.rs::concurrent_first_writes_leave_one_overall_note` |

决策依据：D3、D4，不变量 4。

## Binding a web page must carry its overall note into the article whole

风险：绑定时总批丢失；覆盖文章已有的总批；只有总批的网页违反“未绑定网页不能为空”的约束；合并超限时绑定了一半；清空网页最后的内容后留下空网页。前置：真实 PostgreSQL；一个只有总批的网页，一篇已有总批的文章。触发：绑定；绑定第二个网页到同一篇；绑定一个合并后超限的网页；清空只有总批的网页；删除网页最后一处标注但总批还在。

必须成立：只有总批的网页合法；绑定后网页没有自己的总批，文章总批是原文加 `---` 加网页总批；超限时绑定失败，网页和文章都不变；清空后没有标注的网页记录一起删除；还有总批时删除最后一处标注，网页保留；绑定后写网页总批返回 409 `web_page_bound`，查询返回文章的总批。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 合并、超限回滚、网页生命周期（真实 PostgreSQL） | Covered | `crates/db/tests/postgres/overall_notes.rs::binding_carries_the_overall_note_into_the_article_whole` |
| 插件按绑定状态写到网页或文章（单元测试） | Covered | `apps/palace-extension/src/palace.test.ts` 的 “writes an overall note where the page stands” |

决策依据：D3、D7，不变量 1、5。

## The overall note card must stay out of the reading flow

风险：卡片把正文往下推；没有总批的页面多出入口；写到一半保存失败时丢字；版本冲突时覆盖；窄屏时第一行压住正文；对话切换分支时仍显示上一支的总批。前置：组件测试；文章页与对话页。触发：按 `n` 书写并等待自动保存；保存失败或冲突后 `Esc`；切换分支；在输入框和选区小条显示时按 `n`。

必须成立：没有总批时页面没有卡片；有总批时卡片在阅读页正文之外；`n` 打开编辑，停止输入后自动保存，收起时再保存；失败时卡片保持展开、保留输入并提示；切换 Path 显示那一支的总批；输入框里和小条显示时 `n` 不生效。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 显示、自动保存、失败保留、输入框内不触发（组件测试） | Covered | `apps/palace-web/src/components/overall-note.test.tsx` |
| 切换分支换总批（组件测试） | Covered | `apps/palace-web/src/pages/conversation.test.tsx` 的 “keeps one overall note per branch” |
| 卡片在正文之外、固定在视口底部，窄屏收成小标签（真实 Chromium） | Covered | `apps/palace-web/e2e/overall-note.spec.ts` |

决策依据：D5，不变量 6、8。

## The extension must not lose an overall note it could not save

风险：断网时文字丢失；关闭标签页后草稿消失；草稿在保存成功后残留，下次覆盖服务端内容；草稿覆盖别处新写的内容；内容脚本能写别的文章的总批；总批的快捷键在 Palace 页面生效。前置：service worker 的请求处理，假的 Palace 与本机存储。触发：保存时断网，再打开网页；保存成功；内容脚本发出保存；在 Palace 页面按快捷键。

必须成立：保存前草稿已写入本机，成功后删除；打开网页时草稿优先于服务端内容并标记未保存，保存时用它原来的版本，服务端已有新内容时返回冲突；内容脚本只能按网址保存，不能指定文章；Palace 自己的页面不打开卡片。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 草稿的写入、恢复与清除（单元测试） | Covered | `apps/palace-extension/src/palace.test.ts` 的 “keeps a draft until Palace has it” |
| 真实浏览器里的卡片与 closed shadow root；`chrome.commands` 的快捷键无法由 Playwright 触发，未覆盖 | Covered | `apps/palace-extension/e2e/extension.spec.ts` 的 “the overall note card writes to Palace and survives a failed save” |

决策依据：D6。
