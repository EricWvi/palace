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

## PostgreSQL 与导入事务

`palace-db` 启动时执行 `migrations/`。Owner 使用内部 UUID；email 保留原显示值，以 trim 后小写值检查活跃 Owner 冲突。未知 issuer/subject 不能凭相同 email 接管已有 Owner。所有业务查询显式传入服务端 Owner Scope；复合外键同时约束 Owner、Conversation 和父消息。

导入在事务级 advisory lock 内完成来源定位、最长完全相同前缀复用和 Import 审计记录。幂等键在 Owner 内唯一，相同请求重试返回原结果，换内容复用键返回冲突。再次导入保留已有标题；标题编辑需走独立业务接口。消息结构在数据库中不可改写，父节点必须先存在，防止自引用、多节点环和跨对话引用。

`task test:integration` 显式执行默认 `#[ignore]` 的 testcontainers PostgreSQL 测试。测试先检查本地 `postgres:17-alpine`，不存在即失败，不主动调用镜像拉取。测试沿用 `DOCKER_HOST`，支持指向 Podman 的 Docker API socket；每次使用独立容器和数据库，不依赖开发数据。

## 长期 Session

Session 不设绝对或空闲过期；每次受保护请求查持久化状态，到 24 小时必须复核，时钟回拨也触发复核。refresh/UserInfo 经 `IdentityProvider` 注入，网络调用只持有会话行锁，不持有业务发布锁。并发请求复用已提交的刷新结果。

浏览器 secret 由独立部署密钥、随机 Session UUID 和轮换代次通过 HMAC-SHA256 生成，数据库仅保存摘要；refresh credential 使用 AES-256-GCM 和 Session UUID 关联数据加密。部署必须持久保管同一密钥；密钥不能放入数据库或日志。旧 secret 只在轮换后 30 秒内接受，响应统一返回当前代次，避免并发标签页不断覆盖新 cookie。

退出先提交本地撤销；外部撤销失败留在持久重试队列。Owner/Identity 禁用会触发数据库撤销相关 Session。认证凭证和 Session 不进入业务同步，也不分配业务版本。
