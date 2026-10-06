# 时刻身份与时间线核心测试用例

当前决策：[对话 Moment 卡片字段](../../../decisions/server/moment/20261004-conversation-moment-card-fields.md)，继承[以 `moment` 表承载时刻身份与时间定位](../../../decisions/server/moment/0-moment-supertype-and-typed-details.md)；卡片标题按[标题从 Conversation 移到 Path](../../../decisions/server/conversation/20261005-title-belongs-to-path.md)取自 Path。本决策修改[树与 Path](../../../decisions/server/conversation/20260919-conversation-tree-and-session-paths.md)中“Conversation 是主页面的一张卡片”的约定；对话树、Path 身份与删除规则的既有用例见 [Conversation 树与 Session Path](../conversation/message-tree.md)。

## Every moment must have exactly one detail of its own kind

风险：时间线出现无法打开的空卡片，或一个 Moment 挂上其他类型、其他 Owner 的详情。前置：一个 Owner 已有对话 Moment，另一个 Owner 也有数据。触发：直接 SQL 插入没有详情的 Moment、插入 `kind` 不一致的详情、跨 Owner 引用 Moment，或在事务中先插 Moment 后插详情。

必须成立：事务提交时每个 Moment 恰好有一条 `kind` 一致的详情，二者 `id` 与 `owner_id` 相同；先写 Moment、后写详情的同一事务可以提交。禁止提交孤立 Moment、孤立详情、类型错配或跨 Owner 引用。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 孤立 Moment 与孤立详情在提交时被拒绝，同事务内补齐后可以提交 | Covered | `moments::every_moment_must_have_exactly_one_detail_of_its_own_kind`、`moments::conversation_path_writes_keep_their_moment_in_the_same_transaction`（真实 PostgreSQL） |
| 详情 `kind` 与 Moment 不一致、跨 Owner 引用被数据库拒绝 | Covered | `moments::every_moment_must_have_exactly_one_detail_of_its_own_kind`（真实 PostgreSQL） |

决策依据：D1、D2，不变量 1、2。

## Conversation path writes must keep their moment in the same transaction

风险：导入、追加或删除只更新了 Path 而遗漏 Moment，造成时间线日期错误、残留空卡片或丢失卡片。前置：一个 Conversation 拥有多个 Path，其中部分共享前缀。触发：新建导入、创建分支、追加导入、幂等重试、删除单个 Path、删除 Conversation，以及在这些操作中途失败。

必须成立：新 Path 与其 Moment 同 ID 一起创建；追加导入把 Moment 的 `occurred_at` 更新为本次提交时间且不改变身份；幂等重试不改变 `occurred_at`；删除 Path 或 Conversation 同时删除对应的全部 Moment；任一步失败则 Path 与 Moment 都保持原状。禁止详情表保存另一份定位时间。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 新建导入与分支各创建一个同 ID 的对话 Moment，Session 唯一性保证一个 Session 只有一个 Moment | Covered | `moments::conversation_path_writes_keep_their_moment_in_the_same_transaction`、`paths::concurrent_duplicate_sessions_create_only_one_card`（真实 PostgreSQL） |
| 追加导入把 Moment 移到最后一次续写的时间，幂等重试不移动 | Covered | `moments::conversation_path_writes_keep_their_moment_in_the_same_transaction`（真实 PostgreSQL） |
| 删除 Path、删除 Conversation 后不残留 Moment，失败时整体回滚 | Covered | `moments::conversation_path_writes_keep_their_moment_in_the_same_transaction`、`paths::deletion_preserves_shared_messages_and_owner_boundaries`、`paths::branches_share_prefix_and_failures_roll_back`（真实 PostgreSQL） |

决策依据：D3，不变量 3、4、5、6。

## Day timeline must return moments inside the caller's local day range

风险：跨天边界的 Moment 落到错误日期，同一 Conversation 的分支在时间线上互相遮蔽，或其他 Owner 的 Moment 泄露。前置：一个 Owner 的 Moment 分布在区间边界两侧，同一 Conversation 的两个 Path 落在不同日期，另一个 Owner 在同一区间也有 Moment。触发：以本地时区算出的 `[start, end)` 区间查询时间线，并在追加导入后再次查询。

必须成立：只返回本 Owner、`start <= occurred_at < end` 的 Moment，按 `occurred_at, id` 升序排列；同一 Conversation 的每个 Path 在各自日期分别出现，各自显示自己 Path 的标题；追加导入后该 Moment 只出现在新日期。禁止返回区间外或其他 Owner 的 Moment。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 半开区间边界、排序与 Owner Scope | Covered | `moments::day_timeline_returns_moments_inside_the_callers_range`（真实 PostgreSQL）、`authenticated_http_imports_preserve_scope_and_file_parity`（真实 HTTP/PG） |
| 多 Path 分别落在各自日期，各自显示 Path 标题 | Covered | `moments::day_timeline_returns_moments_inside_the_callers_range`（真实 PostgreSQL） |
| 追加导入后 Moment 从旧日期移到新日期 | Covered | `moments::day_timeline_returns_moments_inside_the_callers_range`（真实 PostgreSQL） |

决策依据：D3、D4。

## Conversation cards must summarize their own path

风险：卡片上的消息数、摘录或标题来自整棵树或其他 Path，与点开后看到的内容对不上；或时间线接口为摘录下发整条长消息。前置：一个 Conversation 有两条共享前缀、长度不同的 Path，其中一条在内部节点结束，另一条只有一条消息的 Conversation；消息含 Markdown、中英文混排和超过 120 个字符的正文，以及一条开头是超过 512 个字符的链接地址的消息。触发：查询这些 Path 所在日期的时间线，编辑标题与来源后再次查询，并追加导入其中一条 Path。

必须成立：每张卡片的 `id` 等于 Path ID，`message_count` 等于以该 Path 打开阅读页时的消息数；`title` 等于该 Path 的当前标题，`source` 等于 Conversation 的当前来源；`excerpt` 是该 Path 从根开始的至多两条消息，只由消息正文的前 512 个字符解析而来，去除 Markdown 标记、折叠空白，按 Unicode 标量截到 120 个字符并在截断时加 `…`；追加导入不改变摘录。禁止卡片字段保存在 `moment` 或其他表中，禁止摘录包含其他 Path 独有的消息。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 消息数与摘录只来自该 Path，包括内部末端的 Path | Covered | `moments::conversation_cards_summarize_their_own_path`（真实 PostgreSQL） |
| 标题来自 Path、来源来自 Conversation，编辑后立即反映 | Covered | `moments::conversation_cards_summarize_their_own_path`（真实 PostgreSQL） |
| 摘录的 Markdown 去除、空白折叠、120 字符截断与单条消息 | Covered | `excerpt::tests::strips_markdown_to_visible_words`、`excerpt::tests::truncates_by_unicode_scalars`（领域单元测试）、`moments::conversation_cards_summarize_their_own_path` |
| 摘录只读取并解析消息正文的前 512 个字符 | Covered | `moments::conversation_cards_summarize_their_own_path`（截点之后的文字不出现） |
| 前端卡片渲染一行与两行摘录 | Partial | `app.test.tsx::marks only 时刻, keeps other sections out of reach, and links cards to the reading page`（两行）；一行摘录没有前端测试 |

决策依据：[对话 Moment 卡片字段](../../../decisions/server/moment/20261004-conversation-moment-card-fields.md) D1–D3，不变量 1–4。

## Migration must turn existing paths into moments without changing identities

风险：升级生成新 ID 导致已有详情链接失效，或丢失、篡改原有发生时间。前置：已有多个 Conversation，其中包含多 Path 的 Conversation。触发：执行引入 `moment` 的迁移。

必须成立：每个 Path 恰好得到一个同 ID 的对话 Moment，`occurred_at` 等于迁移前 Path 的值；Conversation、Path、Message ID 与导入凭据保持不变。禁止为 Conversation 本身生成 Moment。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 有数据升级后 Moment 与 Path 一一对应，ID 与时间不变 | Covered | `moments::migration_turns_paths_into_moments_and_copies_titles`、`paths::migration_retains_existing_linear_conversations`（真实 PostgreSQL，有数据升级） |

决策依据：D3，“风险与为什么不能直接改写”。
