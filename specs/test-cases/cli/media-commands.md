# pal 媒体命令核心测试用例

本文跟踪[pal CLI 根决策](../../decisions/cli/0-pal-cli-uploads-and-deletes-media-with-env-token.md)中凭证处理、请求次数、输出与退出码约定的长期风险。媒体在服务端的存储结果只能以真实 Palace 路由加真实 PostgreSQL 与 MinIO 为直接证据；CLI 自身的请求次数与本地拒绝可以由本机计数 HTTP 服务器直接观察，因为义务针对的是客户端行为，而不是服务端存储语义。所有证据都以子进程运行真实 `pal` 二进制，检查其 stdout、stderr 与退出码。

## The access token must reach only the configured Palace host

### 风险

token 不过期且权限覆盖全部业务数据。CLI 若在错误信息中回显 token、经明文 `http` 发往远端、跟随重定向转发到其他主机，或在配置不合法时仍发出请求，都会扩大泄露面。

### 前置状态

分别准备：`PAL_URL` 或 `PAL_TOKEN` 缺失或为空；`PAL_URL` 为非回环主机的 `http` 地址、带用户名密码或 query；`PAL_TOKEN` 格式不合法；`PAL_URL` 指向一个只返回 3xx 的本机服务器；合法格式但服务端不认识的 token。

### 触发

运行 `pal s3 upload <path>` 或 `pal s3 delete <id>`。

### 必须成立

配置不合法时在发出任何请求前以非 0 退出，stderr 指出变量名；3xx 按失败报告且不请求重定向目标；服务端返回 401 时报告 `authentication_required`。

### 禁止结果

token 出现在 stdout 或 stderr 中；非回环 `http` 地址收到请求；重定向目标收到请求。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 缺失、空值、非回环 `http`、带凭据或 query 的地址与格式错误的 token 在本地被拒绝，且错误不含 token | Covered | `apps/pal/src/config.rs::rejects_unsafe_configuration_without_echoing_the_token`，纯函数单元测试，逐一比较完整错误值与渲染文本 |
| 配置不合法时进程不发出任何请求 | Covered | `apps/pal/tests/local.rs::local_failures_send_nothing_and_keep_stdout_empty`，真实 `pal` 子进程 + 本机计数 HTTP 服务器 |
| 3xx 不被跟随，重定向目标收不到请求 | Covered | `apps/pal/tests/local.rs::redirects_are_reported_and_never_followed`，真实 `pal` 子进程 + 两个本机计数 HTTP 服务器 |
| 服务端拒绝的 token 报告 401 且 stderr 不含 token | Covered | `apps/pal/tests/palace.rs::pal_uploads_and_deletes_media_through_a_real_palace`，真实 Palace 路由 + 真实 PostgreSQL 17 + 真实 MinIO，默认 ignore |

### 决策依据

根决策 D2、D5 及不变量 1、2。

## Each command must send at most one request

### 风险

上传不幂等。CLI 若在 5xx 或连接中断后自动重试，响应丢失时会悄悄产生重复媒体，用户和 CLI 都无法察觉。

### 前置状态

本机 HTTP 服务器对每个请求返回 `503 media_storage_unavailable` 并计数；另准备一个没有监听的本机端口。

### 触发

分别运行一次 `pal s3 upload <path>` 与 `pal s3 delete <id>`。

### 必须成立

服务器对每条命令恰好收到一次请求；CLI 以非 0 退出并报告 `media_storage_unavailable`。连接失败时上传的 stderr 提示上传可能已在服务端完成。

### 禁止结果

同一命令产生第二次请求；失败后 stdout 有输出。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 5xx 后上传与删除各只请求一次 | Covered | `apps/pal/tests/local.rs::server_failures_are_reported_after_a_single_request`，真实 `pal` 子进程 + 本机计数 HTTP 服务器 |
| 连接失败时上传提示可能已完成 | Covered | `apps/pal/tests/local.rs::local_failures_send_nothing_and_keep_stdout_empty`，真实 `pal` 子进程连接未监听端口 |

### 决策依据

根决策 D3、D5 及不变量 4。

## Upload must print exactly the stored media and nothing on failure

### 风险

脚本依赖 stdout 取得媒体 ID。若输出与服务端实际保存的媒体不一致、混入诊断文本，或失败时写入半截内容，脚本会把错误 ID 写进笔记；若 CLI 不声明类型，图片会永久记为 `application/octet-stream`。

### 前置状态

真实 Palace 已配置真实 MinIO，单文件上限为 8 字节；准备扩展名可识别与不可识别的文件，以及超过上限的文件。

### 触发

运行 `pal s3 upload <path>`。

### 必须成立

成功时退出码为 0，stdout 恰好是一行服务端返回的媒体 JSON，`file_name` 是路径最后一段，`content_type` 按扩展名推断、不可识别时为 `application/octet-stream`，且与数据库中的行一致；stderr 为空。超限时退出码为 1，stderr 包含 413 与服务端的字段错误，stdout 为空。

### 禁止结果

stdout 含多行或非 JSON 内容；失败时 stdout 非空；响应中出现对象 key、bucket 或 endpoint。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 成功输出与服务端保存的行一致，类型按扩展名推断 | Covered | `apps/pal/tests/palace.rs::pal_uploads_and_deletes_media_through_a_real_palace`，真实 Palace 路由 + 真实 PostgreSQL 17 + 真实 MinIO，默认 ignore；完整 JSON 与数据库行比较 |
| 超限失败报告 413、stdout 为空 | Covered | 同上 |
| 不可读或不是普通文件的路径在本地失败、不发请求 | Covered | `apps/pal/tests/local.rs::local_failures_send_nothing_and_keep_stdout_empty`，真实 `pal` 子进程 + 本机计数 HTTP 服务器 |

### 决策依据

根决策 D3、D4 及不变量 5、6。

## Delete must succeed only when Palace confirms removal

### 风险

CLI 若把 404 或其他失败当作成功，用户会以为文件已删除；若跨 Owner 的 token 能删除他人媒体，Owner 隔离被破坏。

### 前置状态

真实 Palace 中 Owner A 已上传一个媒体；Owner A 与 Owner B 各有一个 Access Token；另准备未配置对象存储的 Palace。

### 触发

以 B 的 token 删除该媒体；再以 A 的 token 删除两次；向未配置存储的 Palace 删除；以非 UUID 参数删除。

### 必须成立

B 的删除以 1 退出并报告 `not_found`，媒体仍在；A 的第一次删除以 0 退出且 stdout 为空，数据库中不再有该行；第二次以 1 退出并报告 `not_found`；未配置存储时报告 `media_storage_unavailable`；非 UUID 参数以 2 退出且不发请求。

### 禁止结果

任何失败以 0 退出；B 的删除使媒体消失。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 跨 Owner 删除被拒绝且媒体保留，本 Owner 删除成功后重复删除报告 404 | Covered | `apps/pal/tests/palace.rs::pal_uploads_and_deletes_media_through_a_real_palace`，真实 Palace 路由 + 真实 PostgreSQL 17 + 真实 MinIO，默认 ignore |
| 未配置存储时报告 `media_storage_unavailable` | Covered | 同上 |
| 非 UUID 参数以 2 退出且不发请求 | Covered | `apps/pal/tests/local.rs::local_failures_send_nothing_and_keep_stdout_empty`，真实 `pal` 子进程 + 本机计数 HTTP 服务器 |

### 决策依据

根决策 D5、D6 及不变量 3、5。
