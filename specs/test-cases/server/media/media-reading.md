# 媒体读取核心测试用例

当前决策：[正文以 `/api/media/<id>` 引用媒体，读取时 302 到现签 URL](../../../decisions/server/media/20261020-read-media-by-redirect-to-presigned-url.md)。媒体身份、上传与删除顺序的用例见[媒体文件存储](media-storage.md)；本文件只记录读取与引用引入的新承诺。签名 URL 能否取回字节、响应头是否生效只能以真实 MinIO 为直接证据；Owner 隔离与引用检查只能以真实 PostgreSQL 为直接证据。

## Reading media must redirect only within the owner scope

风险：其他 Owner 通过猜测或泄露的 ID 读到媒体；未配置存储时请求挂起或返回 500；对象缺失时 Palace 返回一个看似成功的跳转之外的错误形状。前置：真实 Router、真实 PostgreSQL 与真实 MinIO；Owner A 有一个图片媒体，Owner B 没有；另有一个行存在而对象已删除的媒体。触发：A 以 Session 和 Access Token 分别请求 `GET /api/media/<id>` 并跟随跳转；B 请求同一 ID；请求随机 UUID 与非 UUID；在未配置存储的服务上请求。

必须成立：A 的两种凭据都得到 302，跟随后取回与上传相同的字节；B 与随机 UUID 得到相同的 404，响应不含 A 的任何信息；非 UUID 返回 400；未配置存储返回 `503 media_storage_unavailable`；对象缺失的媒体跟随跳转后得到对象存储的 404。禁止响应体、日志以外的任何地方出现对象 key 或 bucket。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 本 Owner 的 Session 与 Access Token 都得到可取回字节的跳转（真实 MinIO） | Partial | `crates/backend/tests/http.rs::media::media_http_uploads_and_deletes_within_the_owner`（经 `tests/http/articles.rs::exercise_with_media`，浏览器 Session 跟随跳转从 MinIO 取回字节；Access Token 经同一认证中间件，读取接口未单独验证） |
| 其他 Owner 与不存在的 ID 得到相同 404，非 UUID 400（真实 PostgreSQL 与真实 Router） | Covered | `crates/backend/tests/http.rs::media::media_http_uploads_and_deletes_within_the_owner`（经 `exercise_with_media`） |
| 未配置存储返回 503；对象缺失时跟随跳转得到 404 | Partial | `crates/backend/tests/http.rs::authenticated_http_imports_preserve_scope_and_file_parity`（经 `exercise_without_media`，503）、`crates/backend/tests/http.rs::media::media_http_uploads_and_deletes_within_the_owner`（文章删除后旧签名 URL 得到 404；“行存在而对象缺失”的中间状态未单独构造） |

决策依据：D2，不变量 1、2。

## Presigned media URLs must be stable within the hour and safe to open

风险：每次请求签出不同的 URL，浏览器缓存失效，视频与大图被反复下载；窗口末尾签出的 URL 在页面加载中途过期；上传方声明为 `text/html` 的文件在浏览器中被当作页面打开；签名使用容器内部地址，浏览器无法访问。前置：真实 MinIO；可注入的时钟；图片、视频与 `text/html` 媒体各一个；配置与不配置 `PALACE_S3_PUBLIC_ENDPOINT` 两种服务。触发：在同一小时内的两个时刻、跨过整点的两个时刻分别请求同一媒体；在整点前一秒签名并检查有效期；检查各类型跳转目标的响应头；检查两种配置下 `Location` 的主机。

必须成立：同一小时内 `Location` 完全相同，跨过整点后不同；任何签出的 URL 自签发时起至少 1 小时有效；`Cache-Control` 的 `max-age` 不超过到整点的剩余秒数；`image/*`、`video/*`、`audio/*` 以 `inline` 返回，`text/html` 以 `attachment` 返回，`Content-Type` 等于 `media.content_type`；配置 `PALACE_S3_PUBLIC_ENDPOINT` 时 `Location` 使用它，否则使用 `PALACE_S3_ENDPOINT`，写入与删除始终使用后者。只配置 `PALACE_S3_PUBLIC_ENDPOINT` 而未配置存储时启动失败并指出变量名。禁止用 `sleep` 等待跨过整点，以注入的时钟驱动。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 同一小时内 URL 相同、跨整点不同，至少 1 小时有效，`max-age` 不越过整点 | Covered | `crates/media/src/presign.rs::urls_are_stable_within_the_signing_hour`（签名是纯函数，注入时间）、`crates/media/src/presign.rs::signature_matches_the_aws_example`（AWS 文档向量） |
| 非媒体类型以附件下载，`Content-Type` 来自记录（真实 MinIO） | Covered | `crates/media/tests/minio/articles.rs::presigned_urls_fetch_the_stored_bytes`、`crates/backend/tests/http.rs::media::media_http_uploads_and_deletes_within_the_owner`（经 `exercise_with_media`：上传的 `text/html` 经读取接口跳转后以附件下载） |
| 浏览器侧 endpoint 只用于签名；单独配置时启动失败 | Covered | `crates/media/src/config.rs::s3_settings_are_all_or_nothing`、`crates/media/src/config.rs::invalid_values_name_their_variable` |

决策依据：D2、D3，不变量 3、4。

## Referenced media must not be deletable on its own

风险：单独删除了文章引用的媒体，文章的图片永久缺失；或先删了对象、再因外键删行失败，留下引用着缺失对象的文章。前置：真实 PostgreSQL 与真实 MinIO；一篇导入的文章引用一个媒体；另有一个未被引用的媒体。触发：`DELETE /api/media/<被引用媒体 id>`；`DELETE /api/media/<未被引用媒体 id>`。

必须成立：被引用的媒体返回 `409 media_in_use`，对象与行都仍在，跟随读取跳转仍能取回字节；未被引用的媒体照常删除并返回 204。禁止在返回 409 之前删除对象。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 被引用的媒体返回 409 且对象与行都不变（真实 PostgreSQL 与真实 MinIO） | Covered | `crates/media/tests/minio/articles.rs::deleting_an_article_removes_its_objects_before_its_rows`、`crates/backend/tests/http.rs::media::media_http_uploads_and_deletes_within_the_owner`（经 `exercise_with_media`） |
| 未被引用媒体的删除行为不变 | Covered | `crates/media/tests/minio/articles.rs::deleting_an_article_removes_its_objects_before_its_rows`、`crates/media/tests/minio.rs::delete_removes_the_object_first_and_retries_converge` |

决策依据：D4，不变量 5。
