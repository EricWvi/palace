# Palace 开发文档

工程结构、Rust 工具链、workspace 依赖与 lint、Taskfile及日志组件复用
`example-repo`，将 Ora 命名改为 Palace。Rust crate 位于
`crates/<领域>`，包名使用 `palace-` 前缀；应用入口位于 `apps/`。

运行 `task --list` 查看任务；`task format` 格式化 Rust workspace，`task test` 执行全量
lint 与默认测试。容器集成测试默认忽略，显式运行时只使用已有镜像，通过 Docker API
连接 Podman socket。

## 对话输入

`palace-domain` 校验三种来源、单路径消息和来源链接。两种上传入口均将原始 JSON 字节交给 `ImportRequest::parse`；仅保留 `role/content`，不改写空白、换行或空内容。默认限制为 8 MiB 输入、10000 条消息、每条 1 MiB、32 层 JSON；标题最多 1024 字节，幂等键最多 128 字节。错误区分语法、字段与容量，并包含字段路径。

来源 session ID 保持原值，拒绝路径分隔符、查询和片段标记、百分号及控制字符，生成链接时再编码为单独路径段。服务端不会访问来源页面。路径读取检查全部祖先的 Owner、Conversation 与无环条件。
