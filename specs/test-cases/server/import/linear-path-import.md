# 明确目标的线性导入核心测试用例

决策：[分支与追加导入](../../../decisions/server/import/20260919-explicit-branch-and-append-imports.md)，继承根决策的输入、原子性与隔离要求；请求体中的标题按[标题从 Conversation 移到 Path](../../../decisions/server/conversation/20261005-title-belongs-to-path.md) D2 修改。入口与成功落点见[导入入口与成功落点](import-entry.md)。

## Text and file inputs must have identical parsing semantics

风险：入口不同改变原文、角色或容量边界。前置：合法及非法 JSON；触发：JSON 文本和 multipart 提交。

必须成立：统一解析、保留原文、忽略额外消息字段，非法输入整体拒绝，容量限制在写入前生效。禁止跳过坏消息或执行 Markdown。

证据：Covered — `authenticated_http_imports_preserve_scope_and_file_parity`（真实 HTTP/PG）；领域解析测试覆盖样本、空消息、连续角色、深度与大小边界。

## New branches must share an assistant prefix

风险：独立讨论误入同树或重复正文。前置：已有 U1-A1-U2-A2；触发：创建 U1-A1-U3-A3、相同完整路径、仅共享 U1 或完全不相交的分支。

必须成立：完整 assistant 前缀精确复用，其余两个无有效前缀输入拒绝；Session 唯一性全局覆盖同 Owner/来源。禁止跨 Conversation 自动内容合并。

证据：Covered — `paths::branches_share_prefix_and_failures_roll_back`、`paths::deletion_preserves_shared_messages_and_owner_boundaries`（真实 PostgreSQL）。

## Path updates must retain the entire historical prefix

风险：更新或并发过期请求改写历史、时间先于内容提交。前置：已有路径；触发：追加、只改发生时间、截短、改写、旧前缀更新。

必须成立：只接受相同完整历史及其延长；请求必须携带合法标题，成功后替换该 Path 的标题，其他 Path 不变；失败保留所有消息、标题和时间；创建时间不变，成功更新刷新更新时间。禁止修改 Session、来源。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 只接受完整历史的延长，失败保留消息与时间 | Covered | `paths::updates_are_append_only_and_retries_preserve_path_metadata`（真实 PostgreSQL） |
| 拒绝 Session 与来源字段 | Covered | `http/paths.rs::exercise_path_lifecycle`（真实 HTTP/PG，拒绝多余元数据字段） |
| 标题必填，成功时与消息、时间一起替换，失败时保留原标题 | Covered | `paths::updates_are_append_only_and_retries_preserve_path_metadata`（真实 PostgreSQL）、`http/paths.rs::exercise_path_lifecycle` |

## Import receipts and tree mutations must commit atomically

风险：并发重复 Session、失败回执、网络重试产生孤立卡片或时间漂移。前置：并发请求或回执故障注入；触发：创建、分支、更新与重试。

必须成立：全部状态一起提交；幂等摘要包含标题，同键同请求返回原结果且时间、标题不变，同键不同请求（包括只有标题不同）冲突。禁止留下部分消息、Path 或 Conversation。

证据：Covered — `paths::branches_share_prefix_and_failures_roll_back`、`paths::concurrent_duplicate_sessions_create_only_one_card`、`paths::updates_are_append_only_and_retries_preserve_path_metadata`（真实 PostgreSQL），`authenticated_http_imports_preserve_scope_and_file_parity`（跨入口幂等）。
标题进入幂等摘要：`every_target_validates_its_path_title_and_hashes_it`（领域单元测试）与 `paths::updates_are_append_only_and_retries_preserve_path_metadata`（只有标题不同的同键重试返回冲突）。

## Every import must carry a valid path title

风险：分支导入沿用原 Path 标题、无法起新名字；或服务端替调用方补标题，使同一请求在不同时刻得到不同结果。前置：一个已有 Path 的 Conversation。触发：新建导入、分支导入、追加更新分别携带合法标题、缺少标题、纯空白或超过 1024 UTF-8 字节的标题；分支导入额外携带来源字段。

必须成立：三种导入都必须携带合法标题，新建导入写入第一条 Path，分支导入写入新 Path，追加更新替换该 Path 的标题；缺少或不合法时整体拒绝，不创建或修改任何数据；分支导入仍拒绝来源字段。禁止服务端从其他 Path 复制标题作为默认值。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 三种导入写入各自 Path 的标题 | Covered | `paths::metadata_correction_is_atomic_and_preserves_tree_identities`、`paths::updates_are_append_only_and_retries_preserve_path_metadata`、`moments::day_timeline_returns_moments_inside_the_callers_range`（真实 PostgreSQL） |
| 缺少或不合法标题整体拒绝，分支导入拒绝来源 | Covered | `every_target_validates_its_path_title_and_hashes_it`（领域单元测试）、`http/paths.rs::exercise_path_lifecycle`（真实 HTTP/PG） |
| 分支表单默认填入正在阅读的 Path 标题，追加表单填入该 Path 当前标题 | Covered | `components/branch-manager.test.tsx`、`pages/conversation.test.tsx::switches to an imported branch and dates the way back to its occurrence`、`e2e/branches.spec.ts` |

决策依据：[标题从 Conversation 移到 Path](../../../decisions/server/conversation/20261005-title-belongs-to-path.md) D2，不变量 1、6。
