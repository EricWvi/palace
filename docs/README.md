# Palace 开发文档

工程结构、Rust 工具链、workspace 依赖与 lint、Taskfile及日志组件复用
`example-repo`，将 Ora 命名改为 Palace。Rust crate 位于
`crates/<领域>`，包名使用 `palace-` 前缀；应用入口位于 `apps/`。

运行 `task --list` 查看任务；`task format` 格式化 Rust workspace，`task test` 执行全量
lint 与默认测试。容器集成测试默认忽略，显式运行时只使用已有镜像，通过 Docker API
连接 Podman socket。
