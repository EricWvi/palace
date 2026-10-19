# pal 命令行

`pal` 是 Palace 的命令行工具，源码位于 `apps/pal`。它只通过公开 HTTP 接口访问 Palace，以 Owner
Access Token 认证；决策见 [pal CLI 根决策](../../specs/decisions/cli/0-pal-cli-uploads-and-deletes-media-with-env-token.md)。

## 安装与发布

从仓库的 GitHub Releases 下载对应平台的压缩包，解压后将 `pal`（Windows 为 `pal.exe`）放入 `PATH`。
Linux 和 macOS 提供 x86_64、ARM64 版本，Windows 提供 x86_64 版本；Linux 使用 glibc。
每个 Release 附带 `SHA256SUMS`，可在下载目录运行 `sha256sum --ignore-missing -c SHA256SUMS` 校验。

推送 `v*` tag（例如 `v0.4.1`）触发 [Release pal CLI](../../.github/workflows/release-pal.yml) 工作流。
所有平台完成 release 构建和 `--help` 启动检查后，工作流自动生成发布说明并直接发布正式 GitHub Release，
不保留 draft，也不标记为 prerelease。压缩包按 `pal-<tag>-<target>` 命名。CLI 发布与镜像发布工作流独立运行。

## 配置

| 变量        | 说明                                                                                                                                                                                |
| ----------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `PAL_URL`   | Palace 外部地址，例如 `https://palace.example.com`，可带路径前缀。必须是 `https`；回环地址（`localhost`、`127.0.0.0/8`、`::1`）可以用 `http`。不能包含用户名密码、query 或 fragment |
| `PAL_TOKEN` | 在 Web「时刻」页日期菜单的“密钥管理”中生成的 `palace_pat_…` token                                                                                                                   |

访问 Palace 的命令两者都必需（`pal uuid` 和 `pal jottings check` 不需要），只能来自环境变量，不提供命令行参数或配置文件，避免 token 进入 shell 历史或 `ps`。
配置不合法时 `pal` 不发出任何请求。变量使用 `PAL_` 前缀，与服务端的 `PALACE_*` 配置互不干扰。

## 命令

```sh
pal uuid
# 0199f0a0-0000-7000-8000-000000000000

export PAL_URL=https://palace.example.com
export PAL_TOKEN=palace_pat_...

pal s3 upload ./截图.png
# {"id":"0199…","content_type":"image/png","byte_size":1234,"file_name":"截图.png","created_at":1760000000000}

pal s3 delete 0199…

pal jottings check ./jottings
```

- `pal uuid` 在本地生成一个 UUID v7，stdout 只输出小写、带连字符的 UUID 和换行，退出码为 0。
  不需要 `PAL_URL` 和 `PAL_TOKEN`，也不发出请求。
- `pal s3 upload <path-to-file>` 上传一个文件。`file_name` 取路径最后一段，`Content-Type` 按扩展名
  推断，无法推断时为 `application/octet-stream`；类型上传后不可修改。成功时 stdout 恰好是服务端返回的
  一行媒体 JSON，用 `jq -r .id` 取得媒体 ID。大小上限由服务端 `PALACE_MEDIA_MAX_BYTES` 决定。
- `pal s3 delete <uuid-v7>` 删除媒体，成功时没有输出。媒体不存在或不属于该 token 的 Owner 时报告
  `not_found`。

- `pal jottings check <folder>` 用服务端的同一份规则校验札记文件夹（跳过 `.git`），不需要 `PAL_URL` 和
  `PAL_TOKEN`，也不发出请求。全部合格时没有输出；否则 stderr 逐行列出每个问题，退出码为 1。可以作为札记
  仓库的 pre-commit hook，见[札记](../features/jottings/README.md)。

## 失败与重试

| 情形                                                        | 退出码 |
| ----------------------------------------------------------- | ------ |
| 成功                                                        | 0      |
| 用法错误（缺少参数、ID 不是 UUID）                          | 2      |
| 配置错误、文件不可读、网络错误、服务端拒绝（含 3xx 重定向） | 1      |

失败信息写到 stderr，包含 HTTP 状态和 Palace 错误码；stdout 只在成功时有内容。token 不会出现在任何
输出中。

`pal` 不自动重试，也不跟随重定向。上传不幂等：网络中断时上传可能已在服务端完成，此时重新运行会产生
第二个媒体，stderr 会提示这种可能。删除可以放心重新运行，已删除的媒体再次删除报告 `not_found`。

## 开发与测试

`cargo run -p pal -- s3 upload <path>` 在工作区内运行。本地服务端（`task run:server`）监听回环地址，
可以直接用 `http://127.0.0.1:<port>`。该模式下所有请求都归属固定 Owner、不校验 token，
`PAL_TOKEN` 只需符合格式。

`task test` 运行配置校验、错误渲染的单元测试，以及对本机计数服务器的子进程测试；
`task test:integration` 额外运行对真实 Palace 路由、PostgreSQL 与 MinIO 的端到端测试。
