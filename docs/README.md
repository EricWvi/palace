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

## OIDC 协议

`palace-backend` 使用 `openidconnect` 执行 Authelia Authorization Code Flow，申请 `openid profile email offline_access`，启用 S256 PKCE。登录 state、nonce、verifier 在服务端加密持久化；state 绑定独立浏览器 cookie，10 分钟过期且只能消费一次。ID token 必须通过签名、issuer、audience、有效期、nonce 与可选 at_hash 检查；刷新同时检查原 subject 并读取当前 UserInfo email。HTTP 5xx 和网络故障按临时不可用处理。

协议 API 依据 [openidconnect 文档](https://docs.rs/openidconnect/4.0.1/openidconnect/)；Authelia 客户端需按[官方客户端配置](https://www.authelia.com/configuration/identity-providers/openid-connect/clients/)启用 Authorization Code、refresh token、相应 scopes 和 PKCE，使用 confidential client。

## 运行与 HTTP 接口

配置 `PALACE_DATABASE_URL`、`PALACE_ORIGIN`（外部 HTTPS origin）、`PALACE_OIDC_ISSUER`、`PALACE_OIDC_CLIENT_ID`、`PALACE_OIDC_CLIENT_SECRET`、`PALACE_SESSION_KEY`（32 字节随机密钥的标准 base64）。可选 `PALACE_LISTEN` 默认 `127.0.0.1:8080`、`PALACE_TIMEZONE` 默认 `Asia/Shanghai`。由可信反向代理终止 HTTPS 后转发给 server；运行 `task run:server`。

完整的生产与测试环境变量说明见[环境变量](环境变量.md)。

| 接口 | 行为 |
| --- | --- |
| `GET /auth/login`、`GET /auth/callback` | OIDC 登录及回调 |
| `GET /api/me` | 当前 Owner；未认证时返回 401 和登录地址 |
| `POST /auth/logout`、`POST /auth/logout-all` | 当前或全部设备退出，认证服务故障时也可本地退出 |
| `POST /api/import` | JSON 对象：title、source、session_id、history（原始 JSON 文本字符串）、idempotency_key |
| `POST /api/import/file` | multipart 同名字段；history 为文件原始字节 |
| `GET /api/conversations/{id}` | 对话、消息树和受控来源链接 |
| `GET /api/conversations/{id}/paths/{head}` | 验证后的完整祖先路径 |

所有写请求必须携带严格匹配 `PALACE_ORIGIN` 的 Origin。业务请求没有 ownerId 授权参数。安全 cookie 使用 `__Host-` 前缀、Secure、HttpOnly、SameSite=Lax、Path=/，不设置 Domain，持久期 180 天并滚动续期。业务响应为 `application/json` 且禁止缓存；原始 Markdown 作为 JSON 字符串返回，server 不提供 HTML 渲染。展示端必须安全渲染，不能将字符串直接写入 innerHTML。

## 独立记录同步

`POST /api/sync` 接收最多 100 条 `{id, updatedAt, isDeleted, body}`；每条 body 最多 1 MiB。`updatedAt` 为客户端产生的 epoch 毫秒整数。响应逐条返回 `accepted` 或 `retained` 及服务端当前完整记录；等值保留已有值。任何外部 Owner 记录 ID 使整批失败，不接受归属迁移。

`GET /api/sync?cursor=0&limit=100` 按当前 Owner 取增量，limit 为 1–1000。`serverVersion` 及响应 cursor 为十进制字符串，禁止按 JavaScript Number 处理。空页保持游标；墓碑长期保留。有效写入在全局事务锁内取号并提交，数据库触发器也强制该规则；忽略旧值和等值不消耗业务版本。

当前同步对象是独立记录；导入生成的 Conversation/Message/Import 不经这个通用写入口修改或分发。它们的结构同步、正文编辑及级联删除必须先补齐 sync ADR 明确留给后续的多记录协议，避免半棵消息树通过逐记录 LWW 暴露给客户端。

## 客户端实现边界

SQLite 本地持久化由 Android 原生客户端实现；本仓库不再提供 Rust SQLite 副本、客户端同步调度或客户端 HTTP 封装。客户端事务、待同步状态、游标和冲突恢复的实现及验证由客户端负责；当前服务端仍提供上述独立记录同步接口。

## Authelia 契约测试

`task test:contract` 使用已有 `authelia/authelia:4.39.20` 和 testcontainers 验证真实账号登录、用户授权、Authorization Code、UserInfo、refresh 和 revocation。所有测试账号、client、签名密钥和 TLS 文件位于 `crates/backend/tests/fixtures/authelia/` 及 OIDC 单元测试目录，只用于本地独立容器。测试自己注入 fixture CA 和本地 DNS 解析，不修改系统 hosts、不关闭生产 TLS 验证。

Authelia 的 ID token 不必包含 email；Palace 在验证 ID token 后，通过 subject 匹配的 UserInfo 获取当前 email，登录和复核共用该边界。测试实际经过 offline_access 授权页面对应的 consent API，未依赖开发机已有登录或生产账号。

`PUT /api/conversations/{id}/title` 接收 `{ "title": "新标题" }`，原位修改显示标题。校验与导入相同，Owner、source/session_id、消息父链和 Import head 保持不变；该元数据操作尚不参与跨端结构同步。

Session 的内部 ID、Identity 绑定和创建时间不可更新，撤销时间一经写入不能清空。检测到轮换代次与 secret 摘要不一致，或已知旧 secret 在 30 秒宽限结束后再次使用，会持久撤销该 Session。未知随机 secret 只返回未认证。认证服务限流（429）和 5xx 均属于临时失败，不触发身份失败撤销。email 由独立语法校验器检查，再进行冲突规范化。

HTTP 集成测试直接调用 server/PostgreSQL 验证记录上传、增量拉取和墓碑传播，并检查同步响应不包含认证状态、无独立 cookie 的请求无法通过认证。
