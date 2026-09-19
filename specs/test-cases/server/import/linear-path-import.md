# 明确目标的线性导入核心测试用例

决策：[分支与追加导入](../../../decisions/server/import/20260919-explicit-branch-and-append-imports.md)，继承根决策的输入、原子性与隔离要求。

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

必须成立：只接受相同完整历史及其延长；失败保留所有消息和时间；创建时间不变，成功更新刷新更新时间。禁止修改 Session、标题、来源。

证据：Covered — `paths::updates_are_append_only_and_retries_preserve_path_metadata`（真实 PostgreSQL），`http/paths.rs::exercise_path_lifecycle`（真实 HTTP/PG，拒绝多余元数据字段）。

## Import receipts and tree mutations must commit atomically

风险：并发重复 Session、失败回执、网络重试产生孤立卡片或时间漂移。前置：并发请求或回执故障注入；触发：创建、分支、更新与重试。

必须成立：全部状态一起提交；同键同请求返回原结果且时间不变，同键不同请求冲突。禁止留下部分消息、Path 或 Conversation。

证据：Covered — `paths::branches_share_prefix_and_failures_roll_back`、`paths::concurrent_duplicate_sessions_create_only_one_card`、`paths::updates_are_append_only_and_retries_preserve_path_metadata`（真实 PostgreSQL），`authenticated_http_imports_preserve_scope_and_file_parity`（跨入口幂等）。
