# Conversation 树与 Session Path 核心测试用例

当前决策：[Conversation 元数据编辑](../../../decisions/server/conversation/20260919-menu-action-edits-conversation-metadata.md)，继承此前的[树与 Path](../../../decisions/server/conversation/20260919-conversation-tree-and-session-paths.md)。根决策中同 Session 自动合并与不同根路径同树的旧验证义务已被替换。

## Source sessions must uniquely identify owned paths

风险：重复 Session 生成多张卡片或跨 Owner 关联。前置：一个 Owner 已有 Session；触发：再次创建、并发创建或以其他 Owner 操作。

必须成立：同 Owner/来源的 Session 只能对应一个 Path，另一个 Owner 可以独立使用相同 Session；禁止产生孤立 Conversation。只有显式来源纠错可以改变已有 Path 的来源身份，并且纠错后仍满足唯一性。

证据：Covered — `paths::concurrent_duplicate_sessions_create_only_one_card`、`paths::branches_share_prefix_and_failures_roll_back`、`identity_and_database_constraints_isolate_owners`（真实 PostgreSQL）。HTTP Owner 与来源链接：`http/paths.rs::exercise_path_lifecycle`，由认证 HTTP 集成测试调用。

## Metadata correction must atomically preserve conversation tree identities

风险：导入时选错来源后无法纠正，或来源更新只写入 Conversation/部分 Path，造成身份冲突、错误原始链接、树结构变化和 Import 凭据失真。前置：一个 Conversation 拥有多个 Path，目标来源下分别准备无冲突和被其他 Conversation 占用相同 Session ID 的状态。触发：同时更新标题与来源、只改变其中一项、使用非法值、以其他 Owner 更新，或把来源改为存在 Session 冲突的目标。

必须成立：合法更新在一个事务内替换标题及 Conversation/全部 Path 的来源，保留 Conversation、Path、Message ID、Session ID、消息父链、head、计数、时间和历史 Import 凭据；原始链接改用新来源模板。目标来源冲突、非法输入、不存在记录或跨 Owner 请求必须整体失败，标题、来源与链接均不能部分更新。旧来源身份在成功后释放，新来源身份立即参与唯一性检查。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 标题边界和 Owner Scope 拒绝非法更新 | Partial | `title_edits_share_import_validation`、`identity_and_database_constraints_isolate_owners` 只覆盖现有标题更新，尚未覆盖统一元数据接口 |
| Conversation 与全部 Path 的来源原子更新，目标 Session 冲突时完整回滚 | Missing | 待真实 PostgreSQL 测试 |
| 更新保留树内身份、时间及 Import 凭据，并按新来源生成全部 Path 链接 | Missing | 待真实 PostgreSQL 与 HTTP 集成测试 |
| `PUT /api/conversations/{id}` 只接受完整合法元数据，旧标题专用接口移除 | Missing | 待 HTTP 集成测试 |

决策依据：[卡片菜单统一更新 Conversation 标题与来源](../../../decisions/server/conversation/20260919-menu-action-edits-conversation-metadata.md)。

## Card menu metadata editing must refresh every visible projection

风险：保存成功后卡片、搜索、详情或继续对话链接仍使用旧值，或者失败的乐观更新残留在界面。前置：收藏页已加载一个含多个 Path 的 Conversation，并分别准备成功、校验失败和 Session 冲突响应。触发：从卡片菜单打开“编辑会话”，修改标题或来源后保存、取消、重复提交或失败后重试。

必须成立：对话框预填当前标题与来源；取消不发送请求；保存期间不能重复提交。成功后无需刷新即可让卡片、搜索文本、来源图标、无障碍名称、详情和全部原始链接使用新值，且卡片顺序与默认 Path 不变。失败时对话框保留输入并允许重试，所有已提交投影保持或恢复旧值。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 菜单入口、预填、取消、提交中禁用及失败重试符合对话框契约 | Missing | 待 `components/conversation-actions.test.tsx` 或等价组件测试 |
| 成功更新列表、搜索、详情、来源图标和原始链接缓存，失败不残留新值 | Missing | 待 React Query 交互测试 |
| 桌面与手机浏览器可完成编辑，刷新后仍显示服务端持久化值 | Missing | 待真实 Chromium 测试 |

决策依据：[卡片菜单统一更新 Conversation 标题与来源](../../../decisions/server/conversation/20260919-menu-action-edits-conversation-metadata.md)。

## Shared and internal endpoint paths must remain independently manageable

风险：以叶子充当身份，导致内部末端或相同路径的 Session 消失。前置：同树有长路径及两个相同的短路径。触发：删除长路径及其中一个短路径。

必须成立：共享祖先保留，两个短路径拥有独立 ID；删除不能影响其他 Owner；最后一个 Path 通过删除 Conversation 移除。禁止残留没有内容的卡片。

证据：Covered — `paths::deletion_preserves_shared_messages_and_owner_boundaries`（真实 PostgreSQL）。`components/branch-manager.test.tsx` 验证内部节点和相同路径操作；`e2e/branches.spec.ts` 验证实际浏览器投影、移动端边界及文本省略。

## Fork selection must resolve to one real source session

风险：上下游选择拼接出不存在的路径，或“继续对话”跳转错误 Session。前置：两级分叉及相同内部末端；触发：切换上游、下游、内部末端并刷新深链接。

必须成立：选择后续更新时间最新的匹配 Path，其消息与链接保持同一身份；相同末端的 Session 均可选择。禁止保留不匹配的下游选择。

证据：Covered — `pages/conversation.test.tsx`（React 交互），`e2e/branches.spec.ts`（真实 Chromium）。卡片发生时间与默认路径独立：`paths::library_separates_occurrence_order_from_default_path_selection`（真实 PostgreSQL）。

## Migration must preserve existing linear conversation identities

风险：升级更换消息身份或丢失来源链接。前置：已有单路径 Conversation；触发：执行 0007 迁移。

必须成立：Conversation 与 Message ID/正文保持不变，Session、发生时间及末端进入新 Path。禁止猜测新来源身份。

证据：Covered — `paths::migration_retains_existing_linear_conversations`（真实 PostgreSQL，有数据升级）。

## Message parents must remain acyclic and owner scoped

风险：跨树、跨 Owner 或循环父链导致泄露和无法读取。触发：直接 SQL 或损坏的读取输入。

必须成立：父子同树同 Owner，已保存结构不可改写；读取拒绝循环或缺失祖先。角色无需交替。

证据：Covered — `all_scoped_references_and_multirow_cycles_are_rejected`、`identity_and_database_constraints_isolate_owners`（真实 PostgreSQL），`path_requires_acyclic_scoped_ancestors`（领域单元测试）。
