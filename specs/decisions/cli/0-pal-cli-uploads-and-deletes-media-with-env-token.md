---
status: implemented
date: 2026-10-18
---

# 新增 `pal` 命令行工具，以环境变量中的 Access Token 调用媒体接口；首版只提供 `pal s3 upload` 与 `pal s3 delete`

Palace 新增 Rust 命令行工具 `pal`，源码位于 `apps/pal`。首版只有两个子命令：`pal s3 upload <path-to-file>` 把一个本地文件上传为 Palace 媒体，并在 stdout 输出服务端返回的媒体 JSON（其中 `id` 是唯一需要保存的引用）；`pal s3 delete <uuid-v7>` 删除该媒体。CLI 从环境变量 `PAL_URL` 读取 Palace 地址、从 `PAL_TOKEN` 读取 Owner Access Token，以 `Authorization: Bearer` 调用已有的 `POST /api/media` 与 `DELETE /api/media/{id}`，不新增或修改任何服务端接口。

当前为 `implemented`，核心测试用例见[pal 媒体命令](../../test-cases/cli/media-commands.md)。本文件是 `cli` 领域的根决策，没有前序 ADR，承接[Access Token 决策](../server/owner/20261018-owner-access-tokens-for-cli-and-services.md)留给 `cli` 领域的“CLI 命令设计与凭证存放位置”。涉及新 crate `apps/pal`（包名与二进制名均为 `pal`）和工作区成员列表；服务端、Web 与数据库不变。本决策不提供配置文件、登录命令、凭证钥匙串存储、批量操作、媒体读取或列表，也不承担任何既有脚本的迁移。

## 问题与约束

- [媒体根决策](../server/media/0-uuid-v7-media-identity-over-s3-objects.md)已经提供上传与删除接口，但目前只能用 curl 手写 multipart 请求并自行拼接 `Authorization` 头。往 Markdown 笔记中放图片、批量处理旧文件时，需要一个可以直接在 shell 和脚本里使用的入口。
- Access Token 决策规定：CLI 以 `Authorization: Bearer palace_pat_…` 认证，token 不过期、权限覆盖全部业务数据，泄露后在删除前一直有效。CLI 处理 token 的方式不能扩大它的泄露面。
- 媒体上传不幂等（媒体根决策 D1）：响应丢失后重试会产生第二个媒体。删除是可重试收敛的（D5），重试成功后再次删除返回 404。
- 服务端对声明的 `Content-Type` 不校验、不按扩展名猜测，缺失时记为 `application/octet-stream`。类型一旦写入就不可修改，因此 CLI 发送什么类型就决定了该媒体的类型。
- 用户主要是 Eric 本人，在开发机或服务器 shell、cron、CI 中运行；输出要同时适合人读和 `jq` 之类的脚本处理。

## D1：`pal` 是工作区中的独立二进制 crate，只通过公开 HTTP 接口访问 Palace

`apps/pal` 是新的工作区成员，包名和二进制名都是 `pal`（`apps/` 下是可执行程序，`palace-` 前缀约定针对 `crates/` 下的库；命令名本身就是用户接口，包名与之一致最直接）。它只依赖 HTTP 客户端、命令行解析等外部库，以及 `palace-domain` 中 Access Token 的格式定义；不依赖 `palace-backend`、`palace-db`、`palace-media`。

CLI 与服务端之间唯一的契约是 `contracts/openapi.json` 描述的 HTTP 接口。不链接服务端 crate，是为了让 CLI 不能绕过认证中间件直接触达数据库或对象存储，也避免服务端内部类型的改动牵连 CLI 编译。CLI 不为响应声明结构体：上传成功时只确认响应体是 JSON，然后原样转发（见 D4），服务端新增字段不需要修改 CLI；失败时只读取错误体中的 `error` 或 `kind`、`path`、`message`。

## D2：凭证与地址只来自环境变量 `PAL_TOKEN`、`PAL_URL`

| 变量 | 规则 |
| --- | --- |
| `PAL_URL` | Palace 的外部地址，例如 `https://palace.example.com`；必须是 `https` URL，只有回环地址（`localhost`、`127.0.0.0/8`、`::1`）允许 `http`。不得包含用户名密码、query 或 fragment。可以带路径前缀，CLI 按路径段追加 `api/media`，不手动拼接字符串 |
| `PAL_TOKEN` | 完整 Access Token；必须符合签发格式：`palace_pat_` 加 43 个 base64url 字符 |

两者都是必需的，空值视为未设置。缺失或不合法时，CLI 在发出任何网络请求之前失败，错误信息指出变量名，但从不回显 token 的值。不提供 `--token` 或 `--url` 命令行参数，也不读取配置文件。

- **不接受命令行参数传 token**：argv 会进入 shell 历史，并通过 `ps`、`/proc/<pid>/cmdline` 对同机其他用户可见。环境变量只对进程自身及其所有者可见，也是 cron、CI secret、systemd `Environment=`、direnv 和容器的通用注入方式。
- **地址也只用环境变量**：token 与地址总是成对出现，一个 token 只对签发它的那个 Palace 有意义。让两者来自同一处，避免出现“token 来自环境变量、地址来自参数”而把 token 发往错误的主机。
- **拒绝非本机的 `http`**：Bearer token 以明文头发送，经 `http` 发往远端等于在网络上公开一个不过期的全权凭证。本机 `http` 用于本地开发（`task run:server`）。
- **使用 `PAL_` 而不是 `PALACE_` 前缀**：服务端配置全部是 `PALACE_*`（其中 `PALACE_ORIGIN` 恰好也是外部地址）。开发 shell 常同时加载服务端环境，若 CLI 读取 `PALACE_*`，服务端配置会被 CLI 误用，或者给 CLI 设置的变量被服务端读到。以二进制名作为前缀，让两类配置在命名上不重叠。
- **校验 token 格式**：复用 `palace-domain` 中服务端认证时使用的同一格式规则，只做格式检查，不在本地判断 token 是否有效。它让“把别的 secret 放错变量”或复制时截断在发送前就失败，避免把无关凭证发给 Palace。
- **不跟随重定向**：任何 3xx 都按失败报告。跟随重定向会把带 token 的请求转发到 `PAL_URL` 以外的地址，可能降级为 `http`；multipart 上传也无法安全重放。

## D3：`pal s3 upload <path-to-file>` 每次上传一个文件，不自动重试

CLI 读取 `<path-to-file>` 指向的普通文件，以 `multipart/form-data` 发送到 `POST /api/media`，表单只有一个 `file` 字段：

- part 的 filename 取路径的最后一个组成部分（`Path::file_name`），不含目录；无法表示为 UTF-8 时不发送 filename，服务端记为无文件名。目录、不存在或不可读的路径在发出请求前失败。
- part 的 `Content-Type` 按文件扩展名推断（例如 `.png` → `image/png`），无法推断时发送 `application/octet-stream`。服务端不猜类型，如果 CLI 也不猜，所有通过 CLI 上传的图片都会永久记为 `application/octet-stream`，而类型创建后不可修改。扩展名推断只是提示，与服务端“声明类型不是经过验证的属性”的定位一致。
- 不在本地检查大小上限。上限是部署配置（`PALACE_MEDIA_MAX_BYTES`），CLI 无从得知；超限时服务端返回 413，CLI 按 D5 报告。文件以流的形式发送，CLI 不必把整个文件读入内存。
- 一次只接受一个路径。多文件上传由 shell 循环完成，每个文件独立成功或失败，与媒体根决策 D3“单文件请求只有成功或失败”一致。

上传**不自动重试**，包括连接中断和 5xx。上传不幂等：请求可能已在服务端完成、只是响应丢失，自动重试会悄悄产生重复媒体，且 CLI 无法分辨。失败时由用户决定是否重新运行；遗留的孤儿媒体属于媒体根决策“风险”中已接受的情形。

## D4：输出约定——stdout 只放结果，stderr 放诊断

| 命令 | 成功时 stdout | 退出码 |
| --- | --- | --- |
| `pal s3 upload <path>` | 服务端 `201` 响应体原样输出为一行 JSON，以换行结尾，例如 `{"id":"0199…","content_type":"image/png","byte_size":1234,"file_name":"a.png","created_at":1760000000000}` | 0 |
| `pal s3 delete <id>` | 无输出 | 0 |

原样转发服务端 JSON 而不是只打印 `id`：调用方往往同时需要类型、大小和文件名（例如生成 Markdown 图片引用），而 `jq -r .id` 取 ID 同样简单；转发也让服务端新增字段自动可见，不需要修改 CLI。单行 JSON 让 `xargs`、`while read` 逐行处理多次上传的结果。

诊断、错误和进度都写 stderr，stdout 只包含结果，确保 `pal s3 upload a.png > out.json` 在失败时不会写入半截内容。

## D5：`pal s3 delete <uuid-v7>` 在本地校验 ID，失败按服务端错误码报告

`<uuid-v7>` 必须能解析为 UUID，否则按用法错误退出、不发请求；CLI 不额外要求版本号为 7，因为是否存在、是否属于当前 Owner 只能由服务端判断，旧数据将来导入时也未必是 v7。合法时发送 `DELETE /api/media/{id}`，`204` 视为成功。

删除同样不自动重试。删除可重试收敛，但用户手动重跑已经足够；自动重试会让“响应丢失后重试返回 404”这种情形在 CLI 内部出现，反而难以解释。

错误报告统一规则（上传和删除共用）：

| 情形 | 退出码 | stderr |
| --- | --- | --- |
| 用法错误：缺少参数、未知子命令、ID 不是 UUID | 2 | 命令行解析器的用法提示 |
| 环境变量缺失或不合法、文件不可读 | 1 | 指出变量名或路径，不含 token |
| 网络错误（连接失败、TLS 失败、超时） | 1 | 底层错误描述，以及“上传可能已在服务端完成”的提示（仅上传） |
| 服务端返回非成功状态 | 1 | HTTP 状态码，以及响应体中的 `error` 码（如 `authentication_required`、`not_found`、`media_storage_unavailable`）或输入错误的字段与原因 |

`401` 时额外提示检查 `PAL_TOKEN` 是否已在密钥管理中删除，`404` 时说明“不存在或不属于当前 token 的 Owner”，与服务端不区分两者的语义一致。错误输出、日志和 panic 信息中都不包含 token 或 `Authorization` 头。

连接建立设置 10 秒超时，请求整体不设超时：上传耗时取决于文件大小与网络，固定总超时会让慢网络下的大文件必然失败，而用户随时可以中断进程。

## D6：子命令以 `s3` 命名，CLI 侧也只暴露媒体 ID

子命令按用户要求命名为 `s3`，表示“对象存储中的文件”这一用户视角的操作入口；它的输入和输出仍遵守媒体根决策不变量 1：用户只看到并只提交媒体 ID，CLI 不打印对象 key、bucket 或 endpoint，也不提供绕过 Palace 直连 S3 的路径。将来服务端更换对象存储时，`pal s3` 的行为不变。

## 不变量

1. `pal` 只从 `PAL_TOKEN` 读取凭证；token 不出现在 argv、stdout、stderr、日志或任何文件中。
2. 除回环地址外，`pal` 从不以 `http` 发送 token，也只把 token 发往 `PAL_URL` 所指的主机；地址或 token 不合法时不发出任何请求。
3. `pal` 只通过 Palace 公开 HTTP 接口访问数据，不直连数据库或对象存储，也不输出对象 key、bucket 或 endpoint。
4. 一次 `pal s3 upload` 最多发出一次上传请求；一次 `pal s3 delete` 最多发出一次删除请求。
5. 命令成功时退出码为 0，任何失败时退出码非 0；stdout 只在成功时写入结果。
6. `pal s3 upload` 成功时 stdout 恰好是服务端返回的媒体 JSON 一行。

## 为什么不是这些替代方案

| 替代方案 | 优势 | 本次取舍 |
| --- | --- | --- |
| `--token` 命令行参数 | 一次性调用方便，不必导出变量 | 进入 shell 历史并对同机用户可见，违反不变量 1 |
| 配置文件（如 `~/.config/pal/config.toml`）或 `pal login` 写入凭证 | 配置一次长期可用，可支持多个 Palace 配置档 | 需要设计文件权限、配置档切换和与环境变量的优先级；首版只有一个服务端，环境变量已能覆盖 shell、cron、CI。留给后续决策 |
| 系统钥匙串（keyring） | 凭证静态加密 | 服务器、容器和 CI 中通常没有钥匙串服务，无人值守场景反而不可用 |
| 复用服务端的 `PALACE_ORIGIN` 等 `PALACE_*` 变量 | 少记一组变量名 | 开发 shell 中服务端与 CLI 配置互相串用，见 D2 |
| 上传失败自动重试 | 网络抖动时更省心 | 上传不幂等，响应丢失后重试产生重复媒体且无法察觉，违反不变量 4 |
| 上传成功只打印 `id` | 输出最短，`$(pal s3 upload x)` 直接得到 ID | 丢失类型、大小、文件名，调用方要再查；而没有读取接口可查。`jq -r .id` 的代价很小 |
| CLI 不推断 `Content-Type`，全部交给服务端 | 与服务端“不猜类型”完全一致 | 服务端缺失类型时记为 `application/octet-stream` 且不可修改，CLI 上传的图片将永远无法按图片展示 |
| 子命令命名为 `pal media` | 与服务端领域术语 Media 一致，不暴露存储实现名 | 按用户要求使用 `s3`；输入输出仍只有媒体 ID，见 D6。若评审改名，只影响命令行界面 |
| CLI 链接 `palace-media` 直接写 S3 和数据库 | 不依赖服务端在线 | 绕过认证与 Owner Scope，需要在客户端持有数据库与 S3 凭证，违反不变量 3 |

## 风险与为什么不能直接改写

- **环境变量的泄露面**：环境变量会被子进程继承，也可能出现在 CI 日志的环境转储中。CLI 不启动子进程；在 CI 中应通过 secret 机制注入并依赖其遮罩。环境变量仍比 argv 更窄，这是选择它的原因，而不是认为它绝对安全。
- **重复与孤儿媒体**：上传失败后用户手动重跑，可能留下一个已在服务端完成但响应丢失的媒体。CLI 在网络错误时提示这种可能；清理依赖媒体根决策中尚未实现的对账。
- **输出格式即接口**：`pal s3 upload` 的 stdout 一旦被脚本依赖，就不能改成人类可读的多行格式或只输出 ID。将来若需要人类友好的输出，应新增显式选项，而不是改写默认输出。同样，`PAL_URL`、`PAL_TOKEN` 两个变量名发布后应保持；需要改名时在一段时间内同时接受新旧名称，而不是让既有 cron 和 CI 静默失效。
- **扩展名推断表的变化**：推断依赖第三方库的扩展名表，升级可能改变个别扩展名的类型。这只影响之后的新上传，已有媒体的类型不变。

## 本决策未解决的问题

- **配置文件、多配置档与 `pal login`**：多个 Palace 部署或希望不导出环境变量时再决定，需同时决定与环境变量的优先级。
- **媒体读取、下载与列表命令**：服务端尚无读取和列表接口（媒体根决策“未解决的问题”），CLI 无从实现。
- **其他业务命令**：导入对话、笔记等命令按各自需求另行决策；本决策只确立 `pal` 的凭证、输出和错误约定，后续命令应继承 D2、D4、D5。
- **上传进度与大文件**：服务端单文件上限默认 50 MiB，暂不需要进度显示；服务端支持分片或直传后再考虑。
- **分发与安装**：`pal` 如何构建发布（release 产物、`cargo install`）另行决定；当前通过工作区 `cargo run -p pal` 或 `cargo build -p pal` 使用。

## 落地与验收

`apps/pal` 已加入工作区与 Docker 构建上下文（镜像只构建 `palace-server`，但 Cargo 需要全部成员清单）。配置校验与错误渲染由单元测试覆盖；本地拒绝、单次请求与不跟随重定向由真实 `pal` 子进程对本机计数 HTTP 服务器验证，纳入 `task test`；上传输出与数据库行一致、类型推断、超限、无效 token、跨 Owner 删除与重复删除、未配置存储由真实 `pal` 子进程对真实 Palace 路由、PostgreSQL 与 MinIO 验证，纳入 `task test:integration`。用法见[命令行文档](../../../docs/cli/README.md)。服务端、Web 与数据库行为不变。
