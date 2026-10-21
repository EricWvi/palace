# 摘星标注核心测试用例

当前决策：[文章正文与对话消息可以标注](../../../decisions/web/stars/20261021-annotations-on-articles-and-messages.md)。文章本身的导入、编辑和删除见[摘星文章](articles.md)，对话的 Path 删除见[对话消息树](../../server/conversation/message-tree.md)；本文件只记录标注引入的新承诺。数据库约束、级联与触发器只能以真实 PostgreSQL 为直接证据。

## Annotations must stay within the owner scope

风险：标注接口没有按 Owner 过滤，其他 Owner 能列出、修改或删除别人的标注；或者能在别人的文章或消息上新建标注；或者用自己对话的路径加别人的 `message_id`，把标注挂到别人的消息上。前置：真实 Router 与真实 PostgreSQL；Owner A 有一篇带标注的文章和一个带标注的对话，Owner B 有自己的文章和对话。触发：B 对 A 的文章和对话请求 `GET`、`POST` 标注；B 对 A 的标注请求 `PUT`、`DELETE`；B 在自己对话的路径下 `POST` A 的 `message_id`；未登录请求全部标注接口。

必须成立：B 的每个请求都得到与不存在相同的 404，A 的标注不变；在自己对话下引用别人消息的请求返回 404，不写入任何行；未登录的请求被拒绝。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 跨 Owner 的列出、新建、修改、删除都是 404 且不改变数据（真实 PostgreSQL） | Covered | `crates/db/tests/postgres/annotations.rs::annotations_must_stay_within_the_owner_scope` |
| 消息必须属于路径中的对话，且对话属于当前 Owner（真实 PostgreSQL） | Covered | `crates/db/tests/postgres/annotations.rs::annotations_must_stay_within_the_owner_scope`（别人的消息挂在自己的对话下、自己的消息挂在自己另一个对话下） |
| 接口形状、状态码与 OpenAPI 合同一致（真实 Router） | Covered | `crates/backend/tests/http.rs::authenticated_http_imports_preserve_scope_and_file_parity`（经 `tests/http/annotations.rs::exercise_annotations`，响应由 OpenAPI 校验器逐一核对） |

决策依据：D4、D5，不变量 1、2。

## An annotation must keep a style or a note and an immutable anchor

风险：一处标注既没有样式也没有批注，在页面上什么也不显示，却占着「N 处标注」的计数；锚点被修改后指向别的文字；`quote` 与偏移不一致，导致展示时永远无法定位。前置：领域单元测试与真实 PostgreSQL。触发：新建时 `style` 与 `note` 都为 `null`；`note` 为纯空白或超过 16 KiB；`style` 不在六个值之内；`start >= end`；`quote` 为空白、超过 16 KiB，或其 UTF-16 长度不等于 `end - start`（包括含增补平面字符的 `quote`）；把已有标注 `PUT` 成两项都为 `null`；直接在数据库里更新锚点列、标注对象或 Owner。

必须成立：每种不合格输入都返回 400 并指出字段，不写入任何行；`PUT` 只替换 `style` 与 `note`，并刷新 `updated_at`；数据库拒绝修改锚点、标注对象与 Owner，也拒绝两项都为空的行。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 字段校验与错误字段名（领域单元测试） | Covered | `crates/domain/src/annotation.rs::content_needs_a_style_or_a_note`、`crates/domain/src/annotation.rs::anchors_must_agree_with_their_quote`、`crates/backend/tests/http.rs::authenticated_http_imports_preserve_scope_and_file_parity`（经 `exercise_annotations`：缺样式与批注、`quote` 长度不符、`start = end`、空白批注、未知样式、`PUT` 清空两项） |
| 数据库检查约束与触发器拒绝非法行和锚点修改（真实 PostgreSQL） | Covered | `crates/db/tests/postgres/annotations.rs::annotation_rows_must_keep_content_and_anchor`；`PUT` 只替换两项并刷新 `updated_at` 见 `annotations_must_stay_within_the_owner_scope` |

决策依据：D2、D3，不变量 3、5。

## Annotations must disappear with the text they mark

风险：删除文章、删除对话，或删除 Path 清理消息之后，标注仍然留在表里，指向已不存在的文字；或者级联删除没有和标注对象在同一事务中完成，失败时一半标注已删、文章还在。前置：真实 PostgreSQL；一篇带标注的文章；一个对话有两个 Path，共用主干消息、各自有分支消息，主干和两个分支上都有标注。触发：删除文章；删除其中一个 Path；删除整个对话。

必须成立：删除文章后文章上的标注全部消失；删除一个 Path 后，只被它引用的消息上的标注消失，主干和另一分支上的标注保留；删除对话后对话的全部标注消失。标注列表在每一步都只包含仍然存在的文字上的标注。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 文章删除级联删除其标注（真实 PostgreSQL） | Covered | `crates/db/tests/postgres/annotations.rs::annotations_must_disappear_with_the_text_they_mark` |
| Path 删除只删被清理消息上的标注，对话删除删除全部（真实 PostgreSQL） | Covered | `crates/db/tests/postgres/annotations.rs::annotations_must_disappear_with_the_text_they_mark` |

决策依据：D4，不变量 4。

## Marks must land on the text that was selected

风险：选区换算出的偏移与绘制时使用的坐标不一致，标记画到相邻的文字上；代码块的「复制」按钮、图片占位或表格空白让两边计数错开；画上标记、或折叠段落之后，后续选区的偏移整体平移；渲染规则变化后，标注画在了另一处相同的文字上。前置：前端单元测试与组件测试（jsdom 中的真实 `MarkdownContent`）。触发：在包含行内代码、粗体、链接、代码块、图片占位、表格、列表与引用的正文上，选中跨越这些元素的文字并换算锚点，再用插件绘制；在已有标记和折叠状态下再选中同一段文字；构造 `[start, end)` 与 `quote` 不一致的标注，`quote` 在正文中出现零次、一次或多次。

必须成立：绘制出的 `mark` 文字恰好等于 `quote`；同一段选区在无标记、有标记、折叠三种状态下换算出相同的锚点；整体被标注的行内代码、链接与粗体位于 `mark` 内部；相邻片段合并成一个 `mark`；偏移失配时画在离 `start` 最近的 `quote` 出现处，找不到时不画；一个选区跨越两条消息时不弹出标注小条。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| DOM 计数与 hast 计数一致，绘制和折叠不改变坐标（前端单元测试，jsdom 中的真实 `MarkdownContent`） | Covered | `apps/palace-web/src/lib/annotation-paint.test.tsx` 的 “draws marks where the selection was, in every state”、“lifts wholly marked inline elements into one seamless mark” |
| 锚点解析：原位、最近的出现处、找不到时不画（前端单元测试） | Covered | `apps/palace-web/src/lib/annotation-paint.test.tsx` 的 “relocates a quote to its nearest occurrence” |
| 选区到标注的完整流程：小条、快捷键、保存后显示，跨消息的选区不弹出小条（组件测试，请求经 mock fetch） | Covered | `apps/palace-web/src/pages/annotations.test.tsx` 的 “highlights a selection and removes it from its mark”、“annotates one message at a time and folds the others” |

决策依据：D3、D6、D8，不变量 5、6。

## Focus mode must fold only unmarked blocks

风险：「只看标注」折叠掉了含有标记的块，或者保留了整节都没有标记的标题；展开一串折叠段落时连带展开了其他串；退出后页面仍有折叠残留；对话中折叠了带标注的消息。前置：组件测试（jsdom 中的真实 `MarkdownContent`）。触发：在带若干标注的文章上，从菜单或按 `a` 进入只看标注，点击一行「省略 N 段」，再按 `a` 或 `Esc` 退出；对话页上做同样的操作；没有标注时打开菜单、按 `a`。

必须成立：每个含标记的块都保留，连续的无标记块合并为一行并计数正确；有保留内容的小节保留标题，否则标题一起折叠；列表只折叠无标记的项；只展开被点击的那一串；退出后没有折叠行和 `folded` 类；对话里只折叠没有标注的消息；没有标注时菜单项不可用，`a` 不起作用；meta 行在两种状态下分别显示「N 处标注」和「只看 N 处标注 · 显示全文」。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 文章块、标题与列表项的折叠规则（前端单元测试） | Covered | `apps/palace-web/src/lib/annotation-paint.test.tsx` 的 “folds runs of unmarked blocks” |
| 菜单、快捷键、meta 行与对话消息折叠（组件测试） | Covered | `apps/palace-web/src/pages/annotations.test.tsx` 的 “folds everything but the annotations and back”、“offers no focus mode without annotations”、“annotates one message at a time and folds the others” |

决策依据：D7，不变量 6。

## Margin notes must widen the page only while notes exist

风险：没有批注的文章也被推离中心；最后一条批注删除后页面仍保持加宽；窄屏上仍渲染页边栏，挤出横向滚动；批注保存失败后，界面显示的内容与服务端不一致。前置：组件测试；`matchMedia` 按测试注入宽屏或窄屏。触发：宽屏下打开只有高亮的文章，新建第一条批注，再清空它；窄屏下悬停带批注的标记；让保存批注的请求失败。

必须成立：只有高亮时不渲染页边栏、页面不加宽；第一条批注出现后渲染页边栏，页面加宽；清空最后一条批注后页边栏消失；窄屏下不渲染页边栏，悬停时出现浮层卡片；保存失败时标注保持保存前的状态，并显示失败提示。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 页边栏只在有批注且宽屏时出现（组件测试） | Covered | `apps/palace-web/src/pages/annotations.test.tsx` 的 “writes a note in the margin and removes it when emptied”、“highlights a selection and removes it from its mark” |
| 窄屏浮层与保存失败的回退（组件测试） | Covered | `apps/palace-web/src/pages/annotations.test.tsx` 的 “floats a note under its mark on a narrow window”、“keeps the page unchanged when a save fails” |
| 加宽动画与卡片对齐（真实浏览器布局） | Missing | jsdom 没有布局；需要真实 Chromium 回归测试 |

决策依据：D6，不变量 7。
