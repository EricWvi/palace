---
status: implemented
date: 2026-10-18
---

# 媒体文件存入 S3 兼容对象存储，Palace 以服务端生成的 UUID v7 作为稳定身份；首版只提供上传与删除

Palace 的媒体文件（Media）内容保存在 S3 兼容对象存储（MinIO 或任一 S3 实现），PostgreSQL 的 `media` 表保存归属与访问映射。每个媒体文件由 Palace 服务端在上传时生成的 UUID v7 标识，这个 ID 是唯一的对外引用；对象 key、bucket、endpoint 和签名 URL 都是服务端内部细节，客户端和业务正文不得持有。首版只有两个接口：`POST /api/media` 上传单个文件，返回媒体 ID 与元信息；`DELETE /api/media/{id}` 删除对象及记录。读取、展示和引用管理留给后续决策。

当前为 `implemented`，核心测试用例见[媒体文件存储](../../../test-cases/server/media/media-storage.md)。本文件是 `server/media` 的根决策，没有前序 ADR。表设计参考 dashboard 的 `d_media`（见[共享媒体与附件](../../../../docs/legacy/shared/media.md)），但不兼容 dashboard 的接口、URL 形式或签名 URL 续签机制，也不承担旧 dashboard 媒体的导入；D2 只保证将来导入时不必搬迁对象。实现位于 `palace-media`（对象存储与上传删除编排）、`palace-db` 的 `media` 表、`palace-backend` 的媒体接口和 `palace-server` 的配置。

## 问题与约束

- [同步根决策](../sync/0-client-timestamp-lww-and-global-seq.md)把附件和二进制内容的传输、存储与回收列为未解决问题；笔记中的图片已经是 S3 链接，旧 dashboard 的头像、封面和正文图片视频也都在 MinIO 中。Palace 需要自己的媒体身份，后续的正文图片、旧数据导入和展示才有落点。
- dashboard 用 `serial id` 加随机 UUID `link`，key 形如 `2025/01/<unix>_<文件名>`，并把 160 小时有效的签名 URL 存进表里、每天定时续签。key 混入原始文件名会带出 Unicode、长度和路径字符问题；签名 URL 落库形成需要后台维护的过期数据；删除时先软删记录再删对象，批量中的失败项被静默跳过。
- 正文一旦写入某个媒体引用，就会长期保存、跨设备同步、被导出。写进正文的必须是永不变化的标识，否则更换对象存储、bucket 或 key 布局就要改写全部正文。
- 依照 [Owner 根决策](../owner/0-authelia-identity-and-owner-scoped-records.md)，`media` 保存 Owner 数据，必须带 `owner_id NOT NULL`，并且只能在认证上下文给出的 Owner Scope 内读写。
- Palace 主要由 Eric 自用，媒体规模小；可以接受单进程内存缓冲，但不能接受“用户删除了文件、对象仍留在存储中而 Palace 报告成功”。

## D1：媒体 ID 是服务端生成的 UUID v7，是唯一对外身份

上传时服务端生成 UUID v7 作为 `media.id` 并在响应中返回；接口路径、正文引用和未来同步记录都只使用这个 ID。客户端不能指定 ID，表单中出现 `file` 以外的字段（例如 `owner_id`）时请求被拒绝。

- 一个 ID 同时承担主键和对外身份，不需要 `id`/`link` 两列及其映射；Palace 现有表都使用 v7。
- v7 按时间单调，插入局部性好，按 ID 排序约等于按上传时间排序。
- v7 的时间前缀会暴露上传时间。媒体只在 Owner Scope 内可访问，上传时间本身也作为 `created_at` 返回，这一泄露可以接受。ID 难以猜测不构成授权，所有访问仍以 `(owner_id, id)` 定位。

媒体在服务端完成对象写入之后才存在，因此由服务端生成 ID；让客户端预选 ID 会引入“ID 已占用但对象未写入”“同一 ID 两次上传内容不同”等状态。代价是上传不幂等：响应丢失后重试会产生第二个媒体文件，见“风险”。

## D2：对象 key 是内部映射，与 ID 解耦存储

| 列 | 类型 | 规则 |
| --- | --- | --- |
| `id` | `uuid` | 主键，D1 生成 |
| `owner_id` | `uuid NOT NULL` | 引用 `owner(id)`；`reject_owner_change()` 触发器禁止修改 |
| `object_key` | `text NOT NULL` | 对象在 bucket 中的 key；全局唯一、非空 |
| `content_type` | `text NOT NULL` | 上传时确定的 MIME 类型，见 D3 |
| `byte_size` | `bigint NOT NULL` | 对象字节数，非负 |
| `sha256` | `bytea NOT NULL` | 32 字节内容摘要，用于完整性核对与导入去重判断，不用于自动合并 |
| `file_name` | `text NULL` | 客户端提供的原始文件名，只用于展示，不进入 key；空字符串记为无 |
| `created_at` | `timestamptz NOT NULL` | 数据库时钟写入，毫秒精度并带同名 CHECK |

另有 `UNIQUE (owner_id, id)` 供将来 Owner-scoped 复合外键引用。

新对象的 key 是 `<owner_id>/<media_id>`：不含文件名、扩展名或日期，不需要转义，不会冲突；`owner_id` 前缀让按 Owner 枚举、导出和对账可以直接用对象存储的前缀列举完成。key 仍单独存一列而不是由 ID 推导，是为了让旧 dashboard 对象将来以原 key 登记为 Palace 媒体而无需复制，也让以后改变 key 布局时新旧对象可以共存。bucket 和 endpoint 是部署配置，不入库：一个部署只有一个 bucket。

与 `d_media` 相比有意删去的列：

| `d_media` 列 | 处理 | 原因 |
| --- | --- | --- |
| `id serial` + `link uuid` | 合并为 `id uuid` | D1 |
| `creator_id integer` | 改为 `owner_id uuid` | Owner 根决策 |
| `presigned_url`、`last_presigned_time` | 不存 | 签名 URL 是短期派生值，落库只会制造过期数据和续签任务；需要时按 key 现签 |
| `updated_at`、`deleted_at` | 不存 | 媒体内容不可修改，替换文件就是上传新媒体；删除是硬删除，见 D5 |

## D3：上传接口一次接收一个文件，先写对象后写行

`POST /api/media` 接收 `multipart/form-data`，只有一个 `file` 字段；缺少、重复或出现其他字段时返回 400。

- 文件大小上限由 `PALACE_MEDIA_MAX_BYTES` 决定，默认 50 MiB（对齐 dashboard 编辑器的单文件限制），上限本身可以接受；超过返回 413，空文件返回 400。读取时一越过上限就停止缓冲。
- `content_type` 取该 part 的 `Content-Type`，只保留小写的 `type/subtype`，参数被丢弃；缺失或不是两个合法 token 时记为 `application/octet-stream`。不按扩展名猜测，也不限制类型：声明的类型只是提示，不是经过验证的属性。
- 文件名超过 1024 字节返回 400。
- 服务端计算 `sha256` 与字节数，生成 ID 和 key，单次 `PutObject` 写入对象并把类型写到对象元数据；对象写入成功后插入 `media` 行，返回 `201` 和 `id`、`content_type`、`byte_size`、`file_name`、`created_at`（epoch 毫秒）。字段使用与其他业务接口一致的 snake_case。

一次一个文件而不是 dashboard 的多文件 `photos` 字段：多文件上传要么整体原子（对象存储做不到），要么部分成功（违背 Owner 根决策 D4 “批量不静默部分成功”的精神）。单文件让每个请求只有成功或失败，客户端并发发起多个请求即可。

先写对象、后写行，行存在就意味着对象已写成功。插入行失败时服务端尽力删除刚写入的对象；这次删除也失败时只留下无人引用的对象并记录告警日志，见“风险”。

文件完整缓冲在内存中，因为写入前要得到长度和摘要，50 MiB 上限下单用户内存可以接受。媒体路由按自身文件上限加 64 KiB 单独设置请求体上限，不与导入接口共享，避免调整一方时改变另一方的可接受范围。

## D4：对象存储通过 trait 注入，未配置时接口不可用

上传删除编排依赖一个只有写入和删除两个操作的存储 trait，删除不存在的 key 必须视为成功。生产实现是 `object_store` 的 S3 客户端；测试包装真实 MinIO，在指定步骤注入失败。连接参数来自六个环境变量：`PALACE_S3_ENDPOINT`（http 或 https URL）、`PALACE_S3_REGION`、`PALACE_S3_BUCKET`、`PALACE_S3_ACCESS_KEY_ID`、`PALACE_S3_SECRET_ACCESS_KEY`、`PALACE_S3_PATH_STYLE`（`true` 为 path-style，`false` 为 virtual-hosted style）。

这组变量要么全部提供、要么全部不提供，空值视为未提供。全部不提供时服务正常启动，两个媒体接口不读取请求体，直接返回 `503 media_storage_unavailable`；只提供一部分或取值非法时启动失败并指出变量名，避免拼写错误被静默当成“未启用”。启动时只构建客户端、不连接存储。服务端以“已配置 / 未配置”枚举表达这两种状态。

对象存储请求失败同样返回 `503 media_storage_unavailable`，因为它是可重试的不可用，而不是输入错误；数据库失败返回 500。S3 客户端最多重试 2 次、总计 15 秒，避免用户请求等待库默认的数分钟重试。

媒体表不进入同步域，不使用同步全局锁；对象存储请求从不在数据库事务内执行。

## D5：删除先删对象、后删行，失败可重试收敛

`DELETE /api/media/{id}` 在 Owner Scope 内按 `(owner_id, id)` 查找；不存在（包括属于其他 Owner）返回 404。找到后先删除对象，再删除行，返回 204。任一步失败都返回 5xx 且行仍然存在，重试会重新走完全部步骤；S3 删除不存在的 key 也成功，因此重试不会因对象已删除而失败。并发删除使行已不存在时，最后一步视为成功。

反过来先删行、后删对象（dashboard 的顺序）时，对象删除失败会留下一个 Palace 已无法通过任何接口找到的文件：用户以为照片已删除，文件仍在存储里，而且没有记录能驱动重试。本决策让中间状态停在“行还在、对象已没”，这个状态可以通过重试收敛；代价是读取接口（后续决策）可能遇到行存在而对象缺失，必须按 404 处理。

删除是硬删除，不保留墓碑，也不检查正文是否仍引用该媒体。不提供批量删除；多选删除由客户端逐个调用，失败项可单独重试。

## 不变量

1. 媒体文件的对外身份只有 `media.id`；对象 key、bucket、endpoint 和签名 URL 不出现在任何接口响应或业务正文中。
2. `media.id` 由服务端生成，创建后不变；更换对象存储或 key 布局不改变已有 ID。
3. 每一行 `media` 都对应一个已成功写入的对象。
4. 删除接口返回成功时，对应对象已从对象存储中删除。
5. 所有媒体读写都以 `(owner_id, id)` 定位；`owner_id` 只来自认证上下文且创建后不可修改。
6. 媒体内容不可修改；同一 ID 永远指向同一份字节，其 `sha256` 与对象内容一致。
7. 对象存储请求不在数据库事务内执行，媒体写入不使用同步全局锁。

## 为什么不是这些替代方案

| 替代方案 | 优势 | 本次取舍 |
| --- | --- | --- |
| 沿用 dashboard 的 `serial id` + 随机 UUID `link` | 与旧表结构一一对应，导入时少一次转换 | 两个身份需要维护映射；v4 无序，索引局部性差；Palace 其他表都是 v7 |
| 客户端生成 UUID v7 | 上传可按 ID 幂等重试，与同步记录“本地首次创建时生成 ID”一致 | 引入“ID 已占用但对象未写入”和“同 ID 不同内容”的状态；首版接受重试产生孤儿，等媒体进入同步时再评估 |
| 对象 key 直接使用 ID，不存 `object_key` 列 | 少一列，key 可推导 | 旧 dashboard 对象无法原地登记，只能复制；以后无法平滑改变 key 布局 |
| 按内容 sha256 寻址并去重 | 节省存储，重复上传天然幂等 | 删除需要引用计数，跨 Owner 去重会通过“上传是否秒成”泄露其他 Owner 是否有某个文件；个人规模下节省有限 |
| 存储签名 URL 并定时续签（dashboard） | 读取时少一次签名计算 | 签名 URL 是短期派生值，落库产生过期数据和后台任务；签名本身是本地 HMAC 计算，代价很低 |
| 客户端直传 S3（presigned PUT） | 大文件不经过 Palace，无内存压力 | 需要“预登记—确认”两阶段协议、未确认对象的清理和 bucket CORS 配置；首版文件小，不值得 |
| 文件存 PostgreSQL `bytea` 或本地磁盘 | 与元数据同事务，无外部依赖 | 数据库备份膨胀；本地磁盘与容器部署、现有 MinIO 资产不匹配 |
| 多文件一次上传 | 与 dashboard 接口一致，请求数少 | 只能部分成功，见 D3 |
| 删除时先删行再删对象（dashboard） | 行删除在事务内，失败率低 | 对象删除失败会留下无法追踪的文件且接口已报告成功，违反不变量 4 |

## 风险与为什么不能直接改写

- **孤儿对象**：插入行失败且补偿删除也失败、上传响应丢失后客户端重试、进程在写对象和插入行之间崩溃，都会留下对象存储中有、`media` 表中没有的对象（或有行但没有任何正文引用）。它们只占空间，不会被任何接口返回。key 以 `owner_id` 为前缀，后续可以按前缀列举对象并与表对账清理；当前不实现清理任务。
- **内存缓冲**：峰值内存约为并发上传数乘以上限。将来要支持大视频，应改为流式分片上传或客户端直传，而不是简单调大上限。
- **content type 由客户端声明**：不校验内容与类型是否一致。读取接口设计时必须决定是否以 `Content-Disposition: attachment` 或独立域名提供非图片类型，避免在 Palace 同源下渲染用户上传的 HTML/SVG。
- **ID 与 key 一旦写入正文和存储就不能改写**：已有媒体的 `id` 不能重新生成，`object_key` 不能在不移动对象的情况下修改。迁移对象存储时应整体复制 bucket 并保持 key 不变，然后切换配置；需要改变 key 布局时新对象使用新布局，旧行保持原 key，或者先复制对象、再在单条事务中更新 `object_key`、最后删除旧对象。
- **bucket 配置变更**：`PALACE_S3_BUCKET` 指向一个不含既有对象的 bucket 时，所有已登记媒体都会变成“行存在、对象缺失”。这是部署错误，不能通过删除这些行来“修复”。

## 本决策未解决的问题

- **读取与展示**：通过 ID 获取内容的接口（302 到现签 URL 还是由 Palace 代理）、签名有效期、缓存策略、非图片类型的安全展示。这是首版之后最先需要的决策。
- **引用关系与回收**：正文中如何书写媒体引用（例如 `palace-media:<id>`）、删除时是否检查引用、孤儿对象与未引用媒体的对账清理。
- **同步**：媒体元数据是否进入同步域、客户端缓存与离线可用性、删除墓碑。
- **旧 dashboard 媒体导入**：把 `d_media` 的 key 登记为 Palace 媒体、旧 URL 到新 ID 的映射、缺失对象和共享引用的处理，见[迁移核对](../../../../docs/legacy/migration/README.md)。
- **列表、配额和缩略图**：没有媒体列表接口，也不生成缩略图或限制 Owner 的总用量。

## 落地与验收

`media` 表、`palace-media` 的存储 trait 与 S3 实现、上传删除编排、HTTP 接口及配置均已落地，并纳入 OpenAPI 合同。上传与删除的步骤顺序、失败收敛和 Owner 隔离由真实 PostgreSQL 加真实 MinIO 测试验证，其中数据库失败通过测试内创建的触发器注入，对象存储失败通过包装真实 bucket 注入；HTTP 测试经真实路由验证响应形状、上限、跨 Owner 404 和未配置 503。现有导入、时间线、同步和笔记行为不变。

运行 `task test` 验证默认单元测试与 lint；`task test:integration` 显式运行默认忽略的 PostgreSQL 与 MinIO 容器测试。部署配置见[环境变量](../../../../docs/server/环境变量.md#媒体存储)，验证证据见对应领域核心测试用例。
