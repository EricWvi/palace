# 媒体文件存储核心测试用例

本文跟踪[媒体文件根决策](../../../decisions/server/media/0-uuid-v7-media-identity-over-s3-objects.md)中媒体身份、对象与记录的一致性、删除收敛、Owner 隔离和存储配置的长期风险。数据库约束与隔离只能以真实 PostgreSQL 测试为直接证据；对象写入、删除及其幂等性只能以真实 MinIO（或其他 S3 兼容实现）测试为直接证据，内存对象存储只能证明较低层规则，最多标为 `Partial`。

## A media row must exist only after its object is stored

### 风险

上传在对象写入失败或尚未完成时就登记 `media` 行，之后的读取拿到一个指向不存在对象的 ID；或者插入行失败后，客户端仍收到媒体 ID。

### 前置状态

对象存储已配置；分别准备对象写入失败、对象写入成功但插入行失败（注入数据库错误）、两步都成功三种情形。

### 触发

以合法的单文件 multipart 请求调用 `POST /api/media`。

### 必须成立

只有对象写入和行插入都成功时才返回 `201` 和媒体 ID；行中的 `byte_size`、`sha256`、`content_type` 与实际写入对象的字节和元数据一致。对象写入失败时不插入行；插入行失败时返回失败，并尝试删除刚写入的对象。

### 禁止结果

不得存在没有对应对象的 `media` 行；失败的上传不得返回媒体 ID。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 成功上传后行与对象内容、长度、摘要一致 | Covered | `crates/media/tests/minio.rs::upload_registers_rows_only_for_stored_objects`，真实 PostgreSQL 17 + 真实 MinIO，默认 ignore；完整行与对象内容、类型逐一比较 |
| 对象写入失败时不插入行且返回失败 | Covered | `crates/media/tests/minio.rs::upload_registers_rows_only_for_stored_objects`；包装真实 MinIO bucket 在写入步骤注入失败，真实 PostgreSQL 检查无新行 |
| 插入行失败时返回失败并删除已写入对象 | Covered | `crates/media/tests/minio.rs::upload_registers_rows_only_for_stored_objects`；真实 PostgreSQL 触发器注入插入失败，列举真实 MinIO 确认对象已删除 |

### 决策依据

根决策 D3 及不变量 3、6。

## Media identity must be a server-generated stable UUID v7

### 风险

客户端能够指定或影响媒体 ID，或者响应中暴露对象 key、bucket、签名 URL，使正文引用绑定到可变的存储细节。

### 前置状态

对象存储已配置；请求中额外携带 `id`、`owner_id`、`object_key` 等字段。

### 触发

上传文件并检查响应、数据库行和对象 key。

### 必须成立

返回的 `id` 是服务端新生成的 UUID v7，与数据库主键相同；新对象 key 为 `<owner_id>/<media_id>`；响应只包含 `id`、`content_type`、`byte_size`、`file_name`、`created_at`。表单中 `file` 以外的字段（例如 `owner_id`）使请求被拒绝，不影响 ID、归属或 key。

### 禁止结果

响应不得包含对象 key、bucket、endpoint 或签名 URL；客户端提交的 ID 或 key 不得被采用；原始文件名不得进入 key。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| ID 为服务端生成的 UUID v7 且与主键一致 | Covered | `crates/backend/tests/http/media.rs::media_http_uploads_and_deletes_within_the_owner`，真实路由 + PostgreSQL + MinIO；`crates/media/tests/minio.rs::upload_registers_rows_only_for_stored_objects` 比较完整数据库行 |
| 新对象 key 由 owner_id 与 media_id 组成，不含文件名 | Covered | `palace-media::store::tests::object_keys_are_owner_prefixed_media_ids` 固定格式；`crates/media/tests/minio.rs::upload_registers_rows_only_for_stored_objects` 确认行中的 key 与 MinIO 中唯一对象一致 |
| 响应不暴露存储细节，客户端字段不被采用 | Covered | `crates/backend/tests/http/media.rs::media_http_uploads_and_deletes_within_the_owner` 对完整响应深比较并拒绝 `owner_id` 字段；同一测试的真实响应经 OpenAPI 合同校验 |

### 决策依据

根决策 D1、D2 及不变量 1、2。

## Deleting media must remove the object before reporting success

### 风险

删除接口先删行再删对象，对象删除失败时用户以为文件已删除，文件却仍留在存储中，而且没有记录能驱动重试。

### 前置状态

当前 Owner 拥有一个已上传的媒体；分别准备对象删除失败、行删除失败（注入数据库错误）和两步都成功的情形。

### 触发

调用 `DELETE /api/media/{id}`；失败后再次调用。

### 必须成立

两步都成功时返回 `204`，对象和行都不存在。任一步失败时返回 `5xx`，行仍然存在；再次调用会重新删除对象（对象已不存在也视为成功）和行，并返回 `204`。

### 禁止结果

不得在对象仍存在时返回成功；不得出现行已删除而对象残留的结果；重试不得因对象已删除而失败。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 成功删除后对象与行都不存在 | Covered | `crates/media/tests/minio.rs::delete_removes_the_object_first_and_retries_converge`，真实 PostgreSQL + 真实 MinIO；`crates/backend/tests/http/media.rs::media_http_uploads_and_deletes_within_the_owner` 经 HTTP 返回 204 |
| 对象删除失败时行保留并返回失败 | Covered | `crates/media/tests/minio.rs::delete_removes_the_object_first_and_retries_converge`；包装真实 MinIO bucket 在删除步骤注入失败 |
| 行删除失败后重试收敛到 204 | Partial | `crates/media/tests/minio.rs::delete_removes_the_object_first_and_retries_converge` 在服务层用真实 PostgreSQL 触发器注入失败并重试成功；HTTP 层的 204 只由正常删除路径证明 |
| 删除不存在的对象 key 仍视为成功 | Covered | `crates/media/tests/minio.rs::delete_removes_the_object_first_and_retries_converge` 的重试删除已不存在的 MinIO 对象 |

### 决策依据

根决策 D5 及不变量 4。

## Media access must stay within the authenticated owner scope

### 风险

知道另一 Owner 的媒体 ID 即可删除其文件，或者普通更新修改媒体的 `owner_id`。

### 前置状态

Owner A 和 Owner B 各上传一个媒体。

### 触发

Owner B 以 Owner A 的媒体 ID 调用删除接口；直接在数据库中尝试修改 `media.owner_id`。

### 必须成立

Owner B 的删除返回 `404`，与 ID 不存在时的响应一致，Owner A 的行和对象都保留。修改 `owner_id` 被数据库拒绝。

### 禁止结果

不得删除其他 Owner 的对象或行；不得通过响应区分“属于其他 Owner”和“不存在”；`owner_id` 不得在创建后改变。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 跨 Owner 删除返回 404 且不产生副作用 | Covered | `crates/backend/tests/http/media.rs::media_http_uploads_and_deletes_within_the_owner` 比较 404 响应并确认 MinIO 对象数不变；`crates/media/tests/minio.rs::media_stays_within_its_owner` 比较其他 Owner 的 ID 与不存在的 ID |
| `media.owner_id` 创建后不可修改 | Covered | `crates/media/tests/minio.rs::media_stays_within_its_owner`，真实 PostgreSQL 拒绝 UPDATE |

### 决策依据

根决策 D5、[Owner 根决策](../../../decisions/server/owner/0-authelia-identity-and-owner-scoped-records.md) D3、D4 及媒体根决策不变量 5。

## Object storage configuration must be all or nothing

### 风险

部分 S3 环境变量拼写错误或遗漏时，服务静默以“未启用媒体”启动，或者以不完整配置连接存储，直到上传时才暴露问题。

### 前置状态

分别准备 `PALACE_S3_*` 全部提供、全部缺失、只提供一部分三组配置，通过参数注入而不修改进程环境。

### 触发

启动服务并调用媒体接口。

### 必须成立

全部提供时媒体接口可用；全部缺失时服务正常启动，上传和删除返回 `503`；只提供一部分时启动失败并指出缺失的变量。

### 禁止结果

部分配置不得被当作未配置；未配置时媒体接口不得返回成功，也不得改变数据库。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 三种配置状态解析结果正确 | Covered | `palace-media::config::tests::s3_settings_are_all_or_nothing`（纯解析单元测试，通过注入的 lookup 读取，不修改进程环境）；`palace-server` 以 `?` 传播解析错误终止启动 |
| 未配置时媒体接口返回 503 | Covered | `crates/backend/tests/http.rs::authenticated_http_imports_preserve_scope_and_file_parity` 经真实路由调用两个接口，响应经 OpenAPI 合同校验 |

### 决策依据

根决策 D4。

## Upload limits must be enforced by the media route itself

### 风险

媒体接口沿用业务路由共享的请求体上限，调整导入上限时连带改变上传可接受的大小，或超限请求在 HTTP 层以与约定不符的错误被拒。

### 前置状态

对象存储已配置；`PALACE_MEDIA_MAX_BYTES` 设为一个较小的测试值。

### 触发

分别上传恰好等于上限、超过上限一个字节和空文件的请求。

### 必须成立

等于上限的文件上传成功；超过上限返回 `413`；空文件返回 `400`；后两者都不写对象、不插入行。

### 禁止结果

超限或空文件不得留下对象或行；媒体上传上限不得随导入上限变化。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 上限边界与错误码正确 | Covered | `crates/backend/tests/http/media.rs::media_http_uploads_and_deletes_within_the_owner` 以 8 字节上限验证 8 字节成功、9 字节 413、空文件和缺字段 400；`palace-domain::media::tests::upload_bounds_are_checked_per_field` |
| 被拒请求不写对象、不插入行 | Covered | `crates/backend/tests/http/media.rs::media_http_uploads_and_deletes_within_the_owner` 在被拒请求后核对真实 PostgreSQL 行数与 MinIO 对象数 |

### 决策依据

根决策 D3。
