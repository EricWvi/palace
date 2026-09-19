# Conversation 树与 Session Path 核心测试用例

当前决策：[树与 Path](../../../decisions/server/conversation/20260919-conversation-tree-and-session-paths.md)。根决策中同 Session 自动合并与不同根路径同树的旧验证义务已被替换。

## Source sessions must uniquely identify owned paths

风险：重复 Session 生成多张卡片或跨 Owner 关联。前置：一个 Owner 已有 Session；触发：再次创建、并发创建或以其他 Owner 操作。

必须成立：同 Owner/来源的 Session 只能对应一个 Path，另一个 Owner 可以独立使用相同 Session；禁止产生孤立 Conversation。标题编辑不改变来源身份。

证据：Covered — `paths::concurrent_duplicate_sessions_create_only_one_card`、`paths::branches_share_prefix_and_failures_roll_back`、`identity_and_database_constraints_isolate_owners`（真实 PostgreSQL）。HTTP Owner 与来源链接：`http/paths.rs::exercise_path_lifecycle`，由认证 HTTP 集成测试调用。

## Shared and internal endpoint paths must remain independently manageable

风险：以叶子充当身份，导致内部末端或相同路径的 Session 消失。前置：同树有长路径及两个相同的短路径。触发：删除长路径及其中一个短路径。

必须成立：共享祖先保留，两个短路径拥有独立 ID；删除不能影响其他 Owner；最后一个 Path 通过删除 Conversation 移除。禁止残留没有内容的卡片。

证据：Covered — `paths::deletion_preserves_shared_messages_and_owner_boundaries`（真实 PostgreSQL）。树投影与页面操作证据随前端实现补充。

## Migration must preserve existing linear conversation identities

风险：升级更换消息身份或丢失来源链接。前置：已有单路径 Conversation；触发：执行 0007 迁移。

必须成立：Conversation 与 Message ID/正文保持不变，Session、发生时间及末端进入新 Path。禁止猜测新来源身份。

证据：Covered — `paths::migration_retains_existing_linear_conversations`（真实 PostgreSQL，有数据升级）。

## Message parents must remain acyclic and owner scoped

风险：跨树、跨 Owner 或循环父链导致泄露和无法读取。触发：直接 SQL 或损坏的读取输入。

必须成立：父子同树同 Owner，已保存结构不可改写；读取拒绝循环或缺失祖先。角色无需交替。

证据：Covered — `all_scoped_references_and_multirow_cycles_are_rejected`、`identity_and_database_constraints_isolate_owners`（真实 PostgreSQL），`path_requires_acyclic_scoped_ancestors`（领域单元测试）。
