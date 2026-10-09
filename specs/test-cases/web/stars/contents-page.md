# 摘星入口页核心测试用例

当前决策：[摘星入口是目录式列表](../../../decisions/web/stars/0-contents-page-with-kind-column.md)。导航的当前项与不可点击项的通用规则见[文字导航与时刻地址](../navigation/text-nav-and-day-routes.md#navigation-must-mark-only-the-owning-section-and-keep-inert-items-out-of-reach)；阅读页删除后的落点见[对话阅读页](../../server/conversation/reading-page.md#deleting-a-conversation-must-return-to-where-the-reader-came-from)。

决策已 `approved`，尚未实现，以下验证义务暂无证据。

## Stars navigation must open the conversation list and keep unopened kinds inert

风险：摘星仍点不动，或进入后找不到对话列表；尚未开放的笔记、文章可以被点击或 Tab 到，进入不存在的页面。前置：已登录，有若干对话。触发：点击顶部导航的「摘星」；用键盘 Tab 遍历左侧类型列；分别打开 `/conversations` 与 `/conversations/:id`，读取无障碍树。

必须成立：「摘星」是指向 `/conversations` 的链接；在 `/conversations` 与 `/conversations/:id` 上，导航中只有摘星带 `aria-current="page"`。类型列自上而下为「笔记」「文章」「对话」，「对话」带 `aria-current="page"` 并显示条数；「笔记」「文章」照常显示文字，但没有 `href`、不进入 Tab 顺序、不显示条数。禁止隐藏尚未开放的类型，禁止用 `?kind=` 选择类型。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 摘星链接指向 `/conversations`，列表页与阅读页上只有摘星是当前项 | Missing | — |
| 类型列的顺序、当前类型与条数 | Missing | — |
| 尚未开放的类型不是链接、不进入 Tab 顺序、不显示条数 | Missing | — |

决策依据：D1、D2，不变量 1、2。

## Every row must open the path whose title and date it shows

风险：行上写着一个标题，点开却是另一条 Path；或同一 Conversation 的分支被合并成一行，其他分支在目录中消失。前置：一个 Conversation 有两条标题不同、更新时间不同的 Path，另一个 Conversation 只有一条 Path。触发：打开 `/conversations`，点击每一行。

必须成立：列表中有三行，每行显示自己 Path 的标题、Conversation 的来源，以及该 Path 的 `updated_at`（本地时区 `MM.DD`），并按本地时区的年份分组；点击一行进入 `/conversations/:conversation_id?path=<该 Path ID>`，不带 `date`，阅读页页首标题等于行上的标题。禁止一行代表整个 Conversation，禁止行上的标题来自另一条 Path，禁止链接省略 `path` 而交给默认 Path 选择规则。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 同一 Conversation 的每条 Path 各占一行，行上的标题、来源与日期来自该 Path | Missing | — |
| 行的链接携带该 Path 的 ID 且不带 `date`，打开后页首标题与行一致 | Missing | — |
| 日期与年份分组按浏览器本地时区计算 | Missing | — |

决策依据：D3，不变量 3。

## Conversation list must page every path exactly once in order

风险：分页时条目重复或遗漏，浏览途中有新导入时尤其如此；`total` 与实际行数不一致。前置：真实 PostgreSQL；当前 owner 有多于一页（50 条）的 Path，其中若干条 `updated_at` 相同。触发：不带 `q` 逐页请求 `GET /api/conversations` 直到 `next_cursor` 为 `null`；在取完第一页后导入一条新的 Path，再继续取后续页。

必须成立：各页拼接后，当前 owner 的每条 Path 恰好出现一次，顺序与 `updated_at DESC, id DESC` 一致，`updated_at` 相同时按 Path ID 降序；中途导入的新 Path 不使已有条目在两页之间重复或丢失；`total` 等于该 owner 的 Path 总数，且不受 `q` 影响；每项的 `id` 是 Path ID（即对话 Moment 的 ID），`conversation_id`、`title`、`source`、`updated_at` 与该 Path 及其 Conversation 一致。禁止使用偏移量分页，禁止 `items` 中出现同一 Path 两次。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 逐页取完后每条 Path 恰好出现一次，顺序与并列时的次序正确 | Missing | — |
| 翻页途中新增 Path 不造成重复或遗漏 | Missing | — |
| `total` 等于 Path 总数且不受 `q` 影响；各字段来自该 Path 与其 Conversation | Missing | — |

决策依据：D5，不变量 4、5。证据只能来自真实 PostgreSQL。

## Conversation list must stay within the owner

风险：列表或搜索把其他 owner 的 Path、标题或消息带给当前用户。前置：真实 PostgreSQL；两个 owner 各有 Path，其中另一个 owner 的标题与消息都包含同一关键词。触发：以当前 owner 请求 `GET /api/conversations`，分别不带 `q` 和带该关键词。

必须成立：两次响应都只包含当前 owner 的 Path，`total` 只计当前 owner；带关键词时，另一个 owner 的匹配不出现在结果中。禁止任何响应字段泄露其他 owner 的数据。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 列表、`total` 与搜索结果都只来自当前 owner | Missing | — |

决策依据：D5，不变量 6。证据只能来自真实 PostgreSQL。

## Search must run on Enter and match only the path's own title and messages

风险：输入时逐字发请求，或搜到不在这条 Path 上的消息；用户输入的 `%`、`_` 被当作通配符；中文与短关键词搜不到。前置：真实 PostgreSQL，已启用 `pg_trgm` 并建有 `message_content_trgm`；一个 Conversation 的两条分支共享开头的几条消息，各自后续消息不同；另有标题含关键词的 Path、正文含 `%`、`_` 字面量的消息，以及含两个汉字关键词的消息。触发：在页面按 `/`，输入关键词但不按回车；再按回车；分别搜索共享消息中的词、只在一条分支后续消息中的词、标题中的词、`%`、`_`、两个汉字；最后按 Esc。

必须成立：按 `/` 后出现输入框并获得焦点；按回车前不发请求、列表不变；按回车后以替换历史记录的方式写入 `?q=`，结果中的每一行，其 Path 的标题或该 Path 从根到末端至少一条消息包含关键词（不区分大小写的子串）；共享消息中的词使两条分支都出现，只在一条分支后续消息中的词只使那一条出现；标题中的匹配部分被标出；`%`、`_` 只匹配字面量；两个汉字的关键词照常返回正确结果；没有结果时显示“没有找到。”。关键词为空时按回车、或有内容时按 Esc，都移除 `q` 并回到完整列表；内容为空时按 Esc 收起输入框。禁止输入过程中触发搜索，禁止匹配其他 Path 独有的消息。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 只有回车执行搜索并写入 `?q=`，空关键词与 Esc 清除搜索，再按 Esc 收起 | Missing | — |
| 匹配范围是该 Path 的标题与该 Path 上的消息，共享前缀命中所有共享它的 Path | Missing | — |
| `%`、`_`、`\` 按字面量匹配，少于 3 个字符的中文关键词结果正确 | Missing | — |
| 标题匹配高亮，无结果时的提示 | Missing | — |

决策依据：D4，不变量 7。匹配范围、转义与短关键词的义务只能以真实 PostgreSQL 为证据。

## Returning to the list must restore the search and drop stale rows

风险：从阅读页后退时丢失搜索与滚动位置，读者要重新找；或在阅读页删除、改名后，回到列表仍看到已删除的行或旧标题。前置：列表已加载多页并带 `?q=`，滚动到中部；从其中一行进入阅读页。触发：直接按浏览器后退；再次进入，分别删除当前 Path、删除整个对话、编辑标题后再回到列表。

必须成立：后退回到同一组搜索结果、已加载的页面与原来的滚动位置；从摘星进入的阅读页不显示返回链接；删除整个对话后以替换历史记录的方式进入 `/conversations`，后退不会回到已删除的对话；删除单条 Path 或编辑标题、来源后，回到列表时不出现已删除的行，也不显示旧标题。禁止回到列表时停在顶部或丢失 `q`，禁止列表缓存在删除或编辑后继续生效。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 后退恢复 `q`、已加载的页面与滚动位置（真实 Chromium） | Missing | — |
| 没有 `date` 的阅读页删除对话后替换为 `/conversations` | Missing | — |
| 删除 Path、删除对话、编辑元数据后列表缓存失效 | Missing | — |

决策依据：D6。
