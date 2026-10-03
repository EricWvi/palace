# 时刻身份与时间线核心测试用例

当前决策：[以 `moment` 表承载时刻身份与时间定位](../../../decisions/server/moment/0-moment-supertype-and-typed-details.md)。本决策修改[树与 Path](../../../decisions/server/conversation/20260919-conversation-tree-and-session-paths.md)中“Conversation 是主页面的一张卡片”的约定；对话树、Path 身份与删除规则的既有用例见 [Conversation 树与 Session Path](../conversation/message-tree.md)。

## Every moment must have exactly one detail of its own kind

风险：时间线出现无法打开的空卡片，或一个 Moment 挂上其他类型、其他 Owner 的详情。前置：一个 Owner 已有对话 Moment，另一个 Owner 也有数据。触发：直接 SQL 插入没有详情的 Moment、插入 `kind` 不一致的详情、跨 Owner 引用 Moment，或在事务中先插 Moment 后插详情。

必须成立：事务提交时每个 Moment 恰好有一条 `kind` 一致的详情，二者 `id` 与 `owner_id` 相同；先写 Moment、后写详情的同一事务可以提交。禁止提交孤立 Moment、孤立详情、类型错配或跨 Owner 引用。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 孤立 Moment 与孤立详情在提交时被拒绝，同事务内补齐后可以提交 | Missing | — |
| 详情 `kind` 与 Moment 不一致、跨 Owner 引用被数据库拒绝 | Missing | — |

决策依据：D1、D2，不变量 1、2。

## Conversation path writes must keep their moment in the same transaction

风险：导入、追加或删除只更新了 Path 而遗漏 Moment，造成时间线日期错误、残留空卡片或丢失卡片。前置：一个 Conversation 拥有多个 Path，其中部分共享前缀。触发：新建导入、创建分支、追加导入、幂等重试、删除单个 Path、删除 Conversation，以及在这些操作中途失败。

必须成立：新 Path 与其 Moment 同 ID 一起创建；追加导入把 Moment 的 `occurred_at` 更新为本次提交时间且不改变身份；幂等重试不改变 `occurred_at`；删除 Path 或 Conversation 同时删除对应的全部 Moment；任一步失败则 Path 与 Moment 都保持原状。禁止详情表保存另一份定位时间。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 新建导入与分支各创建一个同 ID 的对话 Moment，Session 唯一性保证一个 Session 只有一个 Moment | Missing | — |
| 追加导入把 Moment 移到最后一次续写的时间，幂等重试不移动 | Missing | — |
| 删除 Path、删除 Conversation 后不残留 Moment，失败时整体回滚 | Missing | — |

决策依据：D3，不变量 3、4、5、6。

## Day timeline must return moments inside the caller's local day range

风险：跨天边界的 Moment 落到错误日期，同一 Conversation 的分支在时间线上互相遮蔽，或其他 Owner 的 Moment 泄露。前置：一个 Owner 的 Moment 分布在区间边界两侧，同一 Conversation 的两个 Path 落在不同日期，另一个 Owner 在同一区间也有 Moment。触发：以本地时区算出的 `[start, end)` 区间查询时间线，并在追加导入后再次查询。

必须成立：只返回本 Owner、`start <= occurred_at < end` 的 Moment，按 `occurred_at, id` 升序排列；同一 Conversation 的每个 Path 在各自日期分别出现，卡片标题取自所属 Conversation；追加导入后该 Moment 只出现在新日期。禁止返回区间外或其他 Owner 的 Moment。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 半开区间边界、排序与 Owner Scope | Missing | — |
| 多 Path 分别落在各自日期，共享 Conversation 标题 | Missing | — |
| 追加导入后 Moment 从旧日期移到新日期 | Missing | — |

决策依据：D3、D4。

## Migration must turn existing paths into moments without changing identities

风险：升级生成新 ID 导致已有详情链接失效，或丢失、篡改原有发生时间。前置：已有多个 Conversation，其中包含多 Path 的 Conversation。触发：执行引入 `moment` 的迁移。

必须成立：每个 Path 恰好得到一个同 ID 的对话 Moment，`occurred_at` 等于迁移前 Path 的值；Conversation、Path、Message ID 与导入凭据保持不变。禁止为 Conversation 本身生成 Moment。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 有数据升级后 Moment 与 Path 一一对应，ID 与时间不变 | Missing | — |

决策依据：D3，“风险与为什么不能直接改写”。
