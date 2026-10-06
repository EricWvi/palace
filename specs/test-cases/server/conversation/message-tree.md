# Conversation 树与 Session Path 核心测试用例

当前决策：[标题从 Conversation 移到 Path](../../../decisions/server/conversation/20261005-title-belongs-to-path.md)，继承此前的[对话阅读页](../../../decisions/server/conversation/20261004-reading-page-owns-conversation-actions.md)、[Conversation 元数据编辑](../../../decisions/server/conversation/20260919-menu-action-edits-conversation-metadata.md)与[树与 Path](../../../decisions/server/conversation/20260919-conversation-tree-and-session-paths.md)。阅读页的界面行为见[对话阅读页](reading-page.md)。根决策中同 Session 自动合并与不同根路径同树的旧验证义务已被替换。

## Source sessions must uniquely identify owned paths

风险：重复 Session 生成多张卡片或跨 Owner 关联。前置：一个 Owner 已有 Session；触发：再次创建、并发创建或以其他 Owner 操作。

必须成立：同 Owner/来源的 Session 只能对应一个 Path，另一个 Owner 可以独立使用相同 Session；禁止产生孤立 Conversation。只有显式来源纠错可以改变已有 Path 的来源身份，并且纠错后仍满足唯一性。

证据：Covered — `paths::concurrent_duplicate_sessions_create_only_one_card`、`paths::branches_share_prefix_and_failures_roll_back`、`identity_and_database_constraints_isolate_owners`（真实 PostgreSQL）。HTTP Owner 与来源链接：`http/paths.rs::exercise_path_lifecycle`，由认证 HTTP 集成测试调用。

## Metadata correction must atomically preserve conversation tree identities

风险：导入时选错来源后无法纠正，或来源更新只写入 Conversation/部分 Path，造成身份冲突、错误原始链接、树结构变化和 Import 凭据失真；或标题写到了其他 Path 上。前置：一个 Conversation 拥有多个 Path，目标来源下分别准备无冲突和被其他 Conversation 占用相同 Session ID 的状态。触发：通过 `PUT /api/conversations/{id}/paths/{path_id}/metadata` 同时更新标题与来源、只改变其中一项、使用非法值、以其他 Owner 更新、指定不属于该 Conversation 的 Path，或把来源改为存在 Session 冲突的目标。

必须成立：合法更新在一个事务内替换指定 Path 的标题及 Conversation/全部 Path 的来源，其他 Path 的标题不变，保留 Conversation、Path、Message ID、Session ID、消息父链、head、计数、时间和历史 Import 凭据；原始链接改用新来源模板。目标来源冲突、非法输入、不存在记录、Path 与 Conversation 不匹配或跨 Owner 请求必须整体失败，且响应不区分这几种不存在的情况，标题、来源与链接均不能部分更新。旧来源身份在成功后释放，新来源身份立即参与唯一性检查。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 标题边界和 Owner Scope 拒绝非法更新 | Covered | `title_edits_share_import_validation`、`paths::metadata_correction_is_atomic_and_preserves_tree_identities`（含 Path 与 Conversation 不匹配）、`http/paths.rs::exercise_path_lifecycle` |
| 标题只写入指定 Path，其他 Path 标题不变 | Covered | `paths::metadata_correction_is_atomic_and_preserves_tree_identities`（真实 PostgreSQL）、`http/paths.rs::exercise_path_lifecycle` |
| Conversation 与全部 Path 的来源原子更新，目标 Session 冲突时标题与来源完整回滚 | Covered | `paths::metadata_correction_is_atomic_and_preserves_tree_identities`（真实 PostgreSQL） |
| 更新保留树内身份、时间及 Import 凭据，并按新来源生成全部 Path 链接 | Covered | `paths::metadata_correction_is_atomic_and_preserves_tree_identities`、`http/paths.rs::exercise_path_lifecycle`（真实 PostgreSQL/HTTP） |
| `PUT /api/conversations/{id}/paths/{path_id}/metadata` 只接受完整合法元数据，`PUT /api/conversations/{id}` 移除 | Covered | `http/paths.rs::exercise_path_lifecycle`（真实 HTTP/PG）、`contract_covers_operations_and_special_wire_types`（路由清单） |

决策依据：[卡片菜单统一更新 Conversation 标题与来源](../../../decisions/server/conversation/20260919-menu-action-edits-conversation-metadata.md)；[标题从 Conversation 移到 Path](../../../decisions/server/conversation/20261005-title-belongs-to-path.md) D3，不变量 2、3。

## Each path must own its title independently

风险：分支共用一个标题，在不同日期的时间线上无法区分；或某处仍读取 Conversation 标题，与 Path 标题不一致。前置：一个 Conversation 有三条 Path，`created_at` 依次递增，标题各不相同。触发：读取时间线卡片、阅读页详情与以 Conversation 为单位的列表；编辑第二条 Path 的标题；删除第一条 Path。

必须成立：每条 Path 都有符合标题规则的标题，Conversation 不保存标题；同一 Path 在卡片与阅读页上显示同一标题；编辑一条 Path 的标题不改变其他 Path；Conversation 名称等于 `created_at, id` 最小的现存 Path 的标题，删除该 Path 后改由剩余最早的 Path 提供。禁止保存独立的 Conversation 名称副本。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 数据库约束拒绝缺少或不合法的 Path 标题，`conversation` 没有标题列 | Covered | `paths::metadata_correction_is_atomic_and_preserves_tree_identities`（真实 PostgreSQL） |
| 编辑一条 Path 的标题不影响其他 Path | Covered | `paths::metadata_correction_is_atomic_and_preserves_tree_identities`、`moments::conversation_cards_summarize_their_own_path`（真实 PostgreSQL）、`pages/conversation.test.tsx::renames only the current path and corrects the source of the whole conversation` |
| Conversation 名称取最早的现存 Path，删除后顺延 | Missing | — 对话列表接口已移除，摘星入口页尚未实现，目前没有使用这个名称的读取路径 |

决策依据：[标题从 Conversation 移到 Path](../../../decisions/server/conversation/20261005-title-belongs-to-path.md) D1、D4，不变量 1、2、4、5。

## Title migration must copy the conversation title to every path

风险：迁移遗漏 Path 或在复制前删除旧列，导致标题丢失且无法恢复；或迁移顺带改动 ID 与时间。前置：已有单 Path 与多 Path 的 Conversation，其中包含共享内部末端的 Path。触发：执行把标题移到 Path 的迁移；再模拟复制后 `NOT NULL` 校验失败。

必须成立：每条 Path 的标题等于其所属 Conversation 迁移前的标题；Conversation、Path、Message ID 与 `created_at`、`updated_at`、`occurred_at` 保持不变；任一步失败时整个迁移回滚，`conversation.title` 仍在。禁止留下没有标题的 Path。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 有数据升级后每条 Path 得到原 Conversation 标题，ID 与时间不变 | Covered | `moments::migration_turns_paths_into_moments_and_copies_titles`、`paths::migration_retains_existing_linear_conversations`（真实 PostgreSQL，有数据升级） |

决策依据：[标题从 Conversation 移到 Path](../../../decisions/server/conversation/20261005-title-belongs-to-path.md) D1“迁移”、“风险与为什么不能直接改写”，不变量 7。

## Shared and internal endpoint paths must remain independently manageable

风险：以叶子充当身份，导致内部末端或相同路径的 Session 消失。前置：同树有长路径及两个相同的短路径。触发：删除长路径及其中一个短路径。

必须成立：共享祖先保留，两个短路径拥有独立 ID；删除不能影响其他 Owner；最后一个 Path 通过删除 Conversation 移除。禁止残留没有内容的卡片。

证据：Covered — `paths::deletion_preserves_shared_messages_and_owner_boundaries`（真实 PostgreSQL）。`components/branch-manager.test.tsx::lists every path newest first, telling same-named branches apart by session` 验证分支列表为内部末端和相同路径的 Path 各列一行并各有操作；`e2e/branches.spec.ts` 验证真实浏览器中五条 Path 的列表顺序与窄屏边界。分支列表的规则见[分支管理改为分支列表](../../../decisions/server/conversation/20261006-branch-list-and-update-from-title-menu.md) D2，不变量 3。

## Fork selection must resolve to one real source session

风险：上下游选择拼接出不存在的路径，或“继续对话”跳转错误 Session。前置：两级分叉及相同内部末端；触发：切换上游、下游、内部末端并刷新深链接。

必须成立：选择后续更新时间最新的匹配 Path，其消息与链接保持同一身份；相同末端的 Session 均可选择；选项以 Path 标题命名，读起来相同的选项带 Session ID。禁止保留不匹配的下游选择，禁止两个选项名字相同。

证据：Covered — `pages/conversation.test.tsx`（React 交互），`e2e/branches.spec.ts`（真实 Chromium），`lib/conversation-tree.test.ts::names fork options by path title, telling same-named branches apart by session`（选项命名与重名区分）。卡片发生时间与默认路径独立：`paths::default_path_follows_updates_while_moments_follow_occurrence`（真实 PostgreSQL）。

## Migration must preserve existing linear conversation identities

风险：升级更换消息身份或丢失来源链接。前置：已有单路径 Conversation；触发：执行 0007 迁移。

必须成立：Conversation 与 Message ID/正文保持不变，Session、发生时间及末端进入新 Path。禁止猜测新来源身份。

证据：Covered — `paths::migration_retains_existing_linear_conversations`（真实 PostgreSQL，有数据升级）。

## Message parents must remain acyclic and owner scoped

风险：跨树、跨 Owner 或循环父链导致泄露和无法读取。触发：直接 SQL 或损坏的读取输入。

必须成立：父子同树同 Owner，已保存结构不可改写；读取拒绝循环或缺失祖先。角色无需交替。

证据：Covered — `all_scoped_references_and_multirow_cycles_are_rejected`、`identity_and_database_constraints_isolate_owners`（真实 PostgreSQL），`path_requires_acyclic_scoped_ancestors`（领域单元测试）。
