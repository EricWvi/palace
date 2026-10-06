# 对话阅读页核心测试用例

当前决策：[对话在摘星阅读页阅读与管理](../../../decisions/server/conversation/20261004-reading-page-owns-conversation-actions.md)，菜单项与分支列表见[分支管理改为分支列表](../../../decisions/server/conversation/20261006-branch-list-and-update-from-title-menu.md)，标题归属见[标题从 Conversation 移到 Path](../../../decisions/server/conversation/20261005-title-belongs-to-path.md)。分叉选择的 Path 解析规则沿用 [Conversation 树与 Session Path](message-tree.md#fork-selection-must-resolve-to-one-real-source-session)；返回链接与来源日期见[文字导航与时刻地址](../../web/navigation/text-nav-and-day-routes.md)。

## Reading page header must describe the current path

风险：切换分支后标题、消息数或“继续对话”仍属于上一条 Path，读者看到的元信息与正文对不上，或跳到错误的来源 Session。前置：一个 Conversation 有两条标题、消息数和 Session ID 都不同的 Path，并有一条在内部节点结束的 Path。触发：以 `?path=` 打开阅读页，在分叉句中切换选项，选择“在此结束”，然后刷新。

必须成立：标题、页面 `<title>`、`N 条消息` 与“继续对话”链接都属于当前 Path，切换后同步变化；分叉点显示为“此处分为 K 支：…”一句文字，当前选项有标记，内部末端以“在此结束”或“在此结束 · Session ID”出现；切换只替换历史记录中的 `path`，刷新后仍显示同一条 Path。禁止显示不属于任何 Path 的消息序列，禁止切换 Path 新增历史记录。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 标题、消息数、继续对话链接随当前 Path 变化 | Covered | `pages/conversation.test.tsx::titles, counts and links the current path, and switching paths only replaces the address`、`pages/conversation.test.tsx::defaults to the latest path and resets downstream forks to the latest matching continuation` |
| 分叉句的选项集合、内部末端与当前标记 | Covered | `pages/conversation.test.tsx::deep-links to internal endpoints and distinguishes sessions with identical message paths` |
| 切换以替换方式写回 `path`，刷新后一致 | Covered | `pages/conversation.test.tsx::titles, counts and links the current path, and switching paths only replaces the address`、`e2e/branches.spec.ts`（真实 Chromium 刷新） |

决策依据：D1、D2，不变量 1、2。

## Conversation management must live only in the reading page title menu

风险：会话收藏页或时间线卡片上残留旧的管理入口，同一操作出现两套行为。前置：迁移完成后的应用。触发：在时间线、阅读页中寻找更新、管理分支、编辑与删除入口；从标题菜单的“更新分支”提交一次追加更新。

必须成立：阅读页标题菜单依次为“更新分支”“管理分支”“编辑对话”“删除对话”，分别打开对应对话框；“更新分支”作用于正在阅读的 Path，回填它的标题与发生时间，提交到它的地址，成功后的落点与分支列表里的“更新”相同；时间线卡片没有管理操作，会话收藏页及其卡片菜单不再存在；界面文案使用“对话”而不是“会话”。禁止存在第二个入口。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 标题菜单的四项、顺序与各自打开的对话框 | Covered | `pages/conversation.test.tsx::keeps conversation management in the title menu, destructive last`、`e2e/timeline.spec.ts`、`e2e/branches.spec.ts` |
| “更新分支”更新当前 Path 并按提交日期改返回日期 | Covered | `pages/conversation.test.tsx::updates the branch being read straight from the title menu` |
| 时间线与其他页面不提供管理入口 | Covered | `app.test.tsx::marks only 时刻, keeps other sections out of reach, and links cards to the reading page`（卡片内没有按钮） |

决策依据：D3，不变量 3；20261006 D1，不变量 1、2。

## Editing must update the current path title and the conversation source everywhere

风险：编辑后阅读页或时间线卡片仍显示旧值，或标题被写到同一对话的其他分支上。前置：一个 Conversation 的两条 Path 落在不同日期，阅读页正显示其中一条。触发：在“编辑对话”中同时修改标题和来源并保存；再分别准备校验失败与 Session 冲突的响应。

必须成立：对话框预填当前 Path 的标题与 Conversation 的来源；成功后无需刷新，阅读页标题、元信息行来源、说话人标签与“继续对话”链接使用新值，被编辑 Path 的时间线卡片显示新标题，两张卡片都显示新来源，另一条 Path 的标题不变。失败时对话框保留输入并可重试，各处投影保持旧值。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 预填、保存成功后阅读页各处投影刷新 | Covered | `pages/conversation.test.tsx::renames only the current path and corrects the source of the whole conversation`、`e2e/timeline.spec.ts` |
| 时间线卡片只改被编辑 Path 的标题，全部卡片改来源 | Partial | `moments::conversation_cards_summarize_their_own_path`（真实 PostgreSQL，卡片字段）；保存后时间线缓存失效没有直接测试 |
| 失败保留输入，投影不残留新值 | Partial | `pages/conversation.test.tsx::renames only the current path and corrects the source of the whole conversation`（只覆盖前端校验失败） |

决策依据：D6，不变量 6；标题归属决策 D3、D4。

## Deleting a conversation must return to where the reader came from

风险：删除后停在已不存在的对话上，后退又回到它；或从某天进入的读者被带回今天。前置：分别以带 `date`、不带 `date` 的地址打开同一类阅读页。触发：从标题菜单删除对话并确认；再准备一次删除失败的响应。

必须成立：带 `date` 时以替换历史记录的方式跳到 `/?date=…`，不带时跳到 `/`；跳转后浏览器后退不会回到已删除对话的地址；删除失败时留在原页、显示错误并可重试。禁止删除成功后历史记录的当前条目仍指向该对话。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 按有无 `date` 选择落点，并以替换方式跳转 | Covered | `pages/conversation.test.tsx::after deleting %s, replaces the page with %s`、`e2e/timeline.spec.ts` |
| 删除失败留在原页并可重试 | Missing | — |

决策依据：D4，不变量 4。

## Deleting a path must remove its moment and keep reading the remaining tree

风险：删除 Path 后时间线残留打不开的卡片，或阅读页继续展示已删除的 Path。前置：一个 Conversation 有三条 Path，读者从其中一条 Path 的日期进入。触发：在管理分支中删除读者来时对应的 Path；再删除到只剩一条 Path。

必须成立：删除后阅读页展示剩余 Path 中按选择规则选出的一条，并以替换方式更新 `path`；返回链接仍指向原日期，那一天的时间线上不再有被删除 Path 的卡片；只剩一条 Path 时管理分支不提供删除 Path。禁止留下没有详情的 Moment，禁止阅读页展示已删除的 Path。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 删除 Path 后阅读页切到剩余 Path，返回日期不变 | Covered | `pages/conversation.test.tsx::falls back to a remaining path after deleting the one being read, keeping the day` |
| 原日期的时间线上被删 Path 的卡片消失 | Covered | `moments::conversation_path_writes_keep_their_moment_in_the_same_transaction`（真实 PostgreSQL） |
| 最后一条 Path 只能通过删除对话移除 | Covered | `paths::deletion_preserves_shared_messages_and_owner_boundaries`（真实 PostgreSQL，服务端规则）、`components/branch-manager.test.tsx::keeps the last path undeletable and explains why only when asked`（界面禁用与提示） |

决策依据：D5，不变量 5。
