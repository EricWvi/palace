# 对话收藏

对话是 Palace 的一种内容类型。本文描述当前已实现的收藏、导入与阅读行为；
整体时间线与跨内容类型体验见[产品设计](../../product/README.md)。

后端提供 `GET /api/conversations`，只返回当前 Owner 的会话。每个会话取所有 Path
最大的 `occurred_at`，列表按该时间降序、会话 ID 降序排列。
返回标题、来源、`session_ids/path_count/path_id/occurred_at/head_message_id/message_count`。
`occurred_at` 为 epoch 毫秒；界面按浏览器本地时区展示和输入。

`POST /api/import` 与 `POST /api/import/file` 均要求 `occurred_at`（JSON 整数 / multipart
十进制字符串），范围为公历 0001–9999 年。对话发生时间参与幂等摘要，相同 key 修改时间会
返回 409。对话发生时间保存到 Path，并在 `conversation_import.occurred_at` 记录本次操作值；
`created_at` 由数据库在创建导入记录时自动生成，不接受客户端输入，也不参与列表排序
或幂等摘要。幂等重试保留原记录及其创建时间。

迁移 `0006_conversation_import_times.sql` 将原 `imported_at` 重命名为 `occurred_at`，
保留已有时间值，并新增 `created_at timestamptz NOT NULL DEFAULT now()`。
历史记录未保存真实创建时间，其 `created_at` 统一以迁移执行时间补齐，不能视为原始导入时间。

同一 Owner、来源下不允许重复 Session ID。Conversation 是消息树，Path 是有独立
Session ID 的来源会话；新建分支显式指定 Conversation，更新只允许追加完整历史。
首页打开更新时间最新的 Path；导入成功直接打开本次 Path。路径接口以 Path ID 定位，
不将树中其他分支拼接为聊天。迁移和接口详情见[分支与路径](分支与路径.md)。

会话卡片菜单通过“编辑会话”同时修改标题和消息来源。对话框预填当前值；保存成功后刷新列表与
详情缓存，来源变化会更新全部 Path 的原始链接，但不改变 Session ID、发生时间或卡片排序。
目标来源下存在相同 Session ID 时保留输入并提示冲突，服务端不会写入部分结果。

## 展示与导入边界

- `react-markdown` + GFM 展示 Markdown、表格和代码块；不执行原始 HTML、不加载 Markdown 内的远程图片，危险 URL 由默认 URL 过滤器拒绝。
- 导入 JSON 必须是非空 `[{"role":"user"|"assistant","content":"..."}]` 消息数组；仓库 `examples/` 文件可直接使用，尚不支持任意厂商完整账户导出格式。
- 分支图按需加载 React Flow，以 Dagre 布局；具体阅读与管理交互见[分支与路径](分支与路径.md)。

## 工程入口

[Web 开发与验证](../../frontend/开发与验证.md)提供运行和测试命令；
[API 客户端](../../frontend/API客户端.md)说明请求封装；
[OpenAPI](../../../contracts/openapi.json)提供完整接口字段和响应定义。
