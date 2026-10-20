# pal 文章上传核心测试用例

决策依据：[文章目录上传 ADR](../../decisions/cli/20261020-article-upload-from-directory.md)。
CLI 输入与请求行为由真实文件、真实子进程和本机 HTTP 服务器验证；文章持久化与 Owner 隔离须经过真实 PostgreSQL 与 MinIO。

## Invalid article directories must fail before any request

- 风险：元数据误拼、路径越界、缺失资源在上传部分文件后才暴露。
- 前置状态：准备合法目录及缺失元数据、未知或重复字段、多个正文、非法日期、缺失媒体、符号链接和越界引用目录。
- 触发：执行 `pal article upload <dir>`。
- 必须成立：本地拒绝非零退出，stdout 为空，诊断定位路径或字段，服务器请求数为零，源文件不变。
- 禁止结果：读取目录外资源、猜测元数据或发出部分上传。

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 非法目录、字段、引用和资源在本地失败且不发送请求 | Covered | `apps/pal/tests/articles.rs::invalid_inputs_send_no_requests`，真实子进程、文件系统与计数服务器 |
| 符号链接和目录外路径被拒绝 | Covered | 同文件 `symbolic_links_send_no_requests`（Unix）和 `invalid_inputs_send_no_requests`，真实文件系统及子进程 |

## Article upload must send one complete multipart and preserve unrelated Markdown

- 风险：全文替换破坏代码，重复上传文件，正文与媒体分开保存。
- 前置状态：正文含图片、视频、重复图片、代码、普通链接、外站地址和未引用文件。
- 触发：执行一次上传。
- 必须成立：一个 `POST /api/articles`，首个 part 为 JSON `article`；后续引用文件各一次，名字匹配 `media:` 引用且 MIME 正确；只改变图片地址，不写源文件。
- 禁止结果：请求 `POST /api/media`、上传未引用文件、读取外站资源或改写正文其他字节。

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| Markdown 转换与完整 multipart 一次提交 | Covered | `apps/pal/tests/articles.rs::sends_one_complete_multipart_without_changing_source`，真实 CLI、本机 multipart 解析与完整字段比较 |

## Article results and failures must preserve retry semantics

- 风险：把已有文章当更新、错误输出混入 stdout，或失败后自动重试。
- 前置状态：服务器分别返回 `200`、`201`、重定向、5xx、非法 JSON 和中断响应。
- 触发：运行上传命令。
- 必须成立：两种成功原样输出服务端 JSON；失败非零且 stdout 为空、不重试、不跟随重定向；不确定的提交结果提示同网址可重跑，不含 token。
- 禁止结果：用本地数据替代已有文章响应，或提示文章重跑必然产生重复媒体。

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 两种成功状态、失败输出、单次请求、重定向与不确定结果提示 | Covered | 同文件 `responses_preserve_output_and_retry_semantics`、`interrupted_success_body_reports_uncertain_commit_without_retrying`，真实子进程、计数 HTTP 服务器及截断响应 TCP 服务 |

## Repeated article uploads must converge within each owner

- 风险：媒体与文章归属脱节，重复导入覆盖正文或串入其他 Owner。
- 前置状态：真实 Palace、PostgreSQL、MinIO，两个 Owner 的 Access Token，带本地媒体的文章目录。
- 触发：Owner A 上传后修改本地正文再次上传；Owner B 用同网址上传。
- 必须成立：A 两次得到同一文章及同一媒体，第二次正文不变；B 得到独立文章，不能读取 A 的文章；媒体、文章和 Moment 正确关联。
- 禁止结果：重复行、正文覆盖、跨 Owner 读取或媒体归属错误。

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 真实 CLI 导入、按网址幂等及 Owner 隔离 | Covered | `apps/pal/tests/support/article_palace.rs::exercise`，由 `apps/pal/tests/palace.rs::pal_uploads_and_deletes_media_through_a_real_palace` 执行；真实 Palace、PostgreSQL 17 与 MinIO，已运行通过，默认 ignore |
