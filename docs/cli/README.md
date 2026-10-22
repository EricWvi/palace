# pal 命令行

`pal` 是 Palace 的命令行工具，源码位于 `apps/pal`。它只通过公开 HTTP 接口访问 Palace，以 Owner
Access Token 认证；决策见 [pal CLI 根决策](../../specs/decisions/cli/0-pal-cli-uploads-and-deletes-media-with-env-token.md)。

## 安装与发布

从仓库的 GitHub Releases 下载对应平台的压缩包，解压后将 `pal`（Windows 为 `pal.exe`）放入 `PATH`。
Linux 和 macOS 提供 x86_64、ARM64 版本，Windows 提供 x86_64 版本；Linux 使用 musl 静态链接，
不依赖宿主机 glibc、动态加载器或 OpenSSL，可用于 Alpine 和 `distroless/static`。
Linux 附件的目标名为 `x86_64-unknown-linux-musl`、`aarch64-unknown-linux-musl`；`pal update` 在 Linux 上也选择这些静态包。
每个 Release 附带 `SHA256SUMS`，可在下载目录运行 `sha256sum --ignore-missing -c SHA256SUMS` 校验。
同时附带 `latest.json`，记录 `version`（完整 tag）和 `assets`；每项包含 `target`、`name`、固定到该版本的
`url` 与压缩包的 `sha256`。更新入口为
[`latest.json`](https://github.com/EricWvi/palace/releases/latest/download/latest.json)，使用
[GitHub 的最新发布附件链接](https://docs.github.com/en/repositories/releasing-projects-on-github/linking-to-releases)。

推送 `v*` tag（例如 `v0.4.1`）触发 [Release pal CLI](../../.github/workflows/release-pal.yml) 工作流。
所有平台完成 release 构建和 `--help` 启动检查后，工作流自动生成发布说明并直接发布正式 GitHub Release，
不保留 draft，也不标记为 prerelease。压缩包按 `pal-<tag>-<target>` 命名。CLI 发布与镜像发布工作流独立运行。
`pal --version` 输出 `built on <短 commit hash> (tag: <仓库 tag>)`，例如
`built on a1b2c3d (tag: v0.4.1)`。版本由仓库提交和 tag 标识，不使用 Cargo 工作区的占位版本号，
也不比较 SemVer 大小。发布构建使用触发发布的完整 tag；本地构建若 HEAD 上没有 tag，显示
`tag: untagged`；源码包不带 Git 元数据时 hash 为 `unknown`。

## 配置

| 变量        | 说明                                                                                                                                                                                |
| ----------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `PAL_URL`   | Palace 外部地址，例如 `https://palace.example.com`，可带路径前缀。必须是 `https`；回环地址（`localhost`、`127.0.0.0/8`、`::1`）可以用 `http`。不能包含用户名密码、query 或 fragment |
| `PAL_TOKEN` | 在 Web「时刻」页日期菜单的“密钥管理”中生成的 `palace_pat_…` token                                                                                                                   |

访问 Palace 的命令两者都必需（`pal uuid`、`pal update`、`pal jottings check` 和 `pal tags` 不需要），只能来自环境变量，不提供命令行参数或配置文件，避免 token 进入 shell 历史或 `ps`。
配置不合法时 `pal` 不发出任何请求。变量使用 `PAL_` 前缀，与服务端的 `PALACE_*` 配置互不干扰。

`pal update` 使用标准代理环境变量：`HTTPS_PROXY`、`HTTP_PROXY`、`ALL_PROXY`、`NO_PROXY`，以及各自的
小写形式。更新下载均为 HTTPS，使用 `HTTPS_PROXY` 或回退到 `ALL_PROXY`；`HTTP_PROXY` 只用于 HTTP
目标。`NO_PROXY` 指定的主机会绕过代理。代理地址可以使用 `http://`，通过 CONNECT 隧道访问 HTTPS。

```sh
HTTPS_PROXY=http://127.0.0.1:7890 pal update
```

## 命令

```sh
pal uuid
# 0199f0a0-0000-7000-8000-000000000000

pal update
# Updated pal to v0.4.1

export PAL_URL=https://palace.example.com
export PAL_TOKEN=palace_pat_...

pal s3 upload ./截图.png
# {"id":"0199…","content_type":"image/png","byte_size":1234,"file_name":"截图.png","created_at":1760000000000}

pal s3 delete 0199…

pal jottings check ./jottings

pal tags ./notes
# rust
# cli

pal article upload ./article-directory
```

- `pal uuid` 在本地生成一个 UUID v7，stdout 只输出小写、带连字符的 UUID 和换行，退出码为 0。
  不需要 `PAL_URL` 和 `PAL_TOKEN`，也不发出请求。
- `pal update` 从 `EricWvi/palace` 的最新正式 Release 读取 `latest.json`，自动选择当前平台的压缩包，
  校验 SHA-256、解压到临时目录，再替换当前运行的 `pal` 可执行文件，保留原执行权限。每次运行都安装
  最新正式版，包括当前已是同一版本的情况；成功时输出 `Updated pal to <tag>`，退出码为 0，无需确认。
  不读取 `PAL_URL` 和 `PAL_TOKEN`，不会将 Palace token 发往 GitHub。安装目录需要可写；不支持的平台、
  下载失败、校验失败或压缩包不合法都会以 1 退出，且不替换原程序。Windows 使用支持运行中文件的替换流程。
- `pal s3 upload <path-to-file>` 上传一个文件。`file_name` 取路径最后一段，`Content-Type` 按扩展名
  推断，无法推断时为 `application/octet-stream`；类型上传后不可修改。成功时 stdout 恰好是服务端返回的
  一行媒体 JSON，用 `jq -r .id` 取得媒体 ID。大小上限由服务端 `PALACE_MEDIA_MAX_BYTES` 决定。
- `pal s3 delete <uuid-v7>` 删除媒体，成功时没有输出。媒体不存在或不属于该 token 的 Owner 时报告
  `not_found`。

- `pal jottings check <folder>` 用服务端的同一份规则校验札记文件夹（跳过 `.git`），不需要 `PAL_URL` 和
  `PAL_TOKEN`，也不发出请求。全部合格时没有输出；否则 stderr 逐行列出每个问题，退出码为 1。可以作为札记
  仓库的 pre-commit hook，见[札记](../features/jottings/README.md)。
- `pal tags <path-to-dir>` 递归读取目录下（跳过 `.git`）每个 `.md` 文件 frontmatter 的 `tags`，stdout 每行
  输出一个标签。标签去重、不排序：文件按相对路径顺序读取，标签按首次出现的顺序输出。没有 frontmatter、
  frontmatter 为空或没有 `tags`（含 `tags:` 空值、空列表）的文件被跳过；`draft: true` 的文件照常读取。
  `tags` 必须是字符串列表，frontmatter 不是合法 YAML 映射或 `tags` 写成其他形式时，stderr 报告文件路径与原因，
  退出码为 1。不需要 `PAL_URL` 和 `PAL_TOKEN`，也不发出请求。

## 上传文章目录

`pal article upload <path-to-dir>` 从目录读取 `article.json`、顶层唯一的 `.md` 正文，以及正文引用的本地媒体。
一次 `POST /api/articles` 保存文章与媒体，源目录只读。成功时 stdout 输出服务端返回的文章 JSON，
其中 `id` 是文章 ID；同一 Owner 下相同原文网址已存在时返回旧文章，**不会更新正文或元数据**。

```text
article-directory/
├── article.json
├── post.md
└── assets/
    ├── cover.jpg
    └── demo.mp4
```

`article.json`：

```json
{"title":"文章标题","url":"https://example.com/original","date":"2026-07-03","author":"作者"}
```

`title`、`url`、`date` 必填，`author` 可省略或为 `null`；重复键、未知字段、非法值会在发送前拒绝。
日期允许 `YYYY-MM-DD` 或带时区偏移的 RFC 3339 时间；只写日期时由服务端按 `PALACE_TIMEZONE` 解释。
不从 Markdown 标题、文件名、目录名或文件时间推断元数据，不解析正文 frontmatter。

Markdown 中用 `![封面](assets/cover.jpg)`、`![演示](assets/demo.mp4)` 引用图片或视频，
也可使用 `./assets/`。CLI 只转换真实的行内图片地址，代码块、行内代码、普通链接与其他正文保留原字节；
同一文件重复引用只传一次，未引用文件忽略。正文最多 4 MiB，文件和整个请求的上限由服务端决定。

媒体文件名只能由 `[A-Za-z0-9._-]` 组成，长度 1–128；不支持嵌套资源目录、中文或空格文件名、
百分号编码、query、fragment、绝对路径、`..`、符号链接、引用式本地图片或手写 `media:` / `/api/media/` 地址。
其他本地图片路径在发送前报错。远程图片和 `data:` URI 保留，CLI 不下载，文章页仍显示占位。
HTML 标签和普通附件链接不参与上传。

详见[上传决策](../../specs/decisions/cli/20261020-article-upload-from-directory.md)和[文章 API](../features/articles/README.md)。

## 失败与重试

| 情形                                                                  | 退出码 |
| --------------------------------------------------------------------- | ------ |
| 成功                                                                  | 0      |
| 用法错误（缺少参数、ID 不是 UUID）                                    | 2      |
| 配置错误、文件不可读、网络错误、服务端拒绝（含 3xx 重定向）、更新失败 | 1      |

失败信息写到 stderr；Palace 接口错误包含 HTTP 状态和 Palace 错误码；stdout 只在成功时有内容。token 不会出现在任何
输出中。

`pal` 不自动重试。访问 Palace 时不跟随重定向；`pal update` 仅通过 HTTPS 下载，允许 GitHub 附件下载所需的
重定向。`pal s3 upload` 不幂等：网络中断时上传可能已在服务端完成，此时重新运行会产生
第二个媒体，stderr 会提示这种可能。删除可以放心重新运行，已删除的媒体再次删除报告 `not_found`。

`pal article upload` 则按 Owner 和原文网址幂等：断线或响应不确定时，可保持相同 `url` 重跑；
已有文章不会被覆盖。修改本地正文后重跑不会更新远端；确需替换时先明确删除旧文章及媒体，再重新导入，
文章 ID 和导入时间会改变。在 Web 修改已有文章网址后，旧网址不再命中该文章。

## 开发与测试

`cargo run -p pal -- s3 upload <path>` 在工作区内运行。本地服务端（`task run:server`）监听回环地址，
可以直接用 `http://127.0.0.1:<port>`。该模式下所有请求都归属固定 Owner、不校验 token，
`PAL_TOKEN` 只需符合格式。

`task test` 运行配置校验、错误渲染的单元测试，以及对本机计数服务器的子进程测试；
`task test:integration` 额外运行对真实 Palace 路由、PostgreSQL 与 MinIO 的端到端测试。
`cargo test -p pal` 也覆盖更新清单、两种压缩格式、校验失败和替换边界；Unix 上通过复制出的测试进程
验证真实自替换与权限保留。`node --test scripts/pal-release-manifest.test.mjs` 校验发布清单生成。
