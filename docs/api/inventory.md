# HTTP 接口与传输语义盘点

盘点日期：2026-09-20。本文记录第一阶段确认的现有行为，作为后续 OpenAPI 建模基线；尚未生成生产接口契约，也不表示已验证所有 HTTP 分支。

CI 检查由根 `Taskfile.yml` 提供统一入口；GitHub Actions 只做构建，不承载本计划的 lint、测试或契约检查编排。

## 接口清单

来源：[实际 Router](../../crates/backend/src/http.rs)、[业务 handler](../../crates/backend/src/http/business.rs)、[路径管理](../../crates/backend/src/http/path_management.rs)、[同步](../../crates/backend/src/http/sync.rs)、[认证](../../crates/backend/src/http/handlers.rs)。共 17 个显式注册的方法与路径组合；Axum 对 GET 自动提供的 HEAD 行为不另算业务操作。静态资源、未知路径和方法不支持的 fallback 不属于这 17 项。

下表的 `J` 表示 `application/json`，`T` 表示 `text/plain; charset=utf-8`。所有 UUID 路径参数都按 UUID 解析。状态码列是具体输入、业务和基础设施错误路径的并集，不保证每种数据状态都能触发其中每项。

鉴权约定：`S` 为 `__Host-palace-session` cookie；`O` 为唯一且精确匹配配置 origin 的 `Origin` 请求头；`L` 为 `__Host-palace-login` cookie 和持久化的一次性 state。`S` 边界可能产生 J 401/409/500/503，写请求先检查 `O`，失败为 J 403。认证后的业务响应添加 session cookie 与 `Cache-Control: no-store`。本地 fixed-user server 使用相同业务路由、固定 Owner 和 O 检查，不提供生产认证路由。

| 方法与路径 | 输入 | 成功响应 | handler / extractor 失败（另加鉴权错误） | 鉴权 | 当前 Web 调用点 |
| --- | --- | --- | --- | --- | --- |
| GET `/api/me` | 无 | 200 J `Owner` | 无业务错误分支 | S | 未使用 |
| GET `/api/sync` | query `cursor`、`limit` 均必填 | 200 J `SyncPage` | 400 J（limit 值域）/T（query 解析），500 J | S | 未使用 |
| POST `/api/sync` | J `Record[]` | 200 J `UploadResult[]` | 400/413/415/422 T；404/409/500 J | S+O | 未使用 |
| POST `/api/import` | J `TextImport` | 200 J `ImportResult` | 400/413/404/409/500 J | S+O | 未使用 |
| POST `/api/import/file` | multipart 六字段，见下文 | 200 J `ImportResult` | 400 T（boundary）；400/413/404/409/500 J | S+O | `import-dialog.tsx`：新会话，粘贴和文件都走此入口 |
| GET `/api/conversations` | 无 | 200 J `Summary[]` | 409/500 J | S | `lib/api.ts` → library |
| GET `/api/conversations/{id}` | UUID `id` | 200 J `Detail` | 400 T；400/404/409/500 J | S | `pages/conversation.tsx`、`branch-manager.tsx` |
| PUT `/api/conversations/{id}` | UUID `id`；J `{title, source}` | 200 J `{id, title, source}` | 400 T；400/404/409/500 J | S+O | `conversation-actions.tsx` |
| DELETE `/api/conversations/{id}` | UUID `id` | 200 J `{id}` | 400 T；404/409/500 J | S+O | `conversation-actions.tsx` |
| POST `/api/conversations/{id}/paths` | UUID `id`；J `NewPath` | 200 J `ImportResult` | 400 T；400/413/404/409/500 J | S+O | `import-dialog.tsx`：新增分支 |
| GET `/api/conversations/{id}/paths/{path_id}` | 两个 UUID | 200 J `Message[]`，根到指定 head | 400 T；400/404/409/500 J | S | 未使用，Web 从详情在本地选择路径 |
| PUT `/api/conversations/{id}/paths/{path_id}` | 两个 UUID；J `UpdatePath` | 200 J `ImportResult` | 400 T；400/413/404/409/500 J | S+O | `import-dialog.tsx`：追加 |
| DELETE `/api/conversations/{id}/paths/{path_id}` | 两个 UUID | 200 J `{id: path_id}` | 400 T；404/409/500 J | S+O | `branch-manager.tsx` |
| GET `/auth/login` | 无 | 303，空 body，`Location` 到 IdP，设置 L cookie | 401/409/500/503 J | 无已有 session 要求 | `error-state.tsx` 的登录链接 |
| GET `/auth/callback` | query `code`、`state` 均必填 | 303，空 body，`Location: /`，设置 S、清除 L cookie，no-store | 400 T；401/409/500/503 J | L；可携带旧 S | IdP 浏览器回跳 |
| POST `/auth/logout` | 无 body | 200 J `{logged_out: true}`，清除 S cookie | 401/403/500 J | S+O | 未使用 |
| POST `/auth/logout-all` | 无 body | 200 J `{logged_out: true}`，清除 S cookie | 401/403/500 J | S+O | 未使用 |

会话查询在数据库解码时可能返回 `DbError::Conflict`；表中保留这种现有失败路径。logout 使用持久化 session 撤销逻辑，而非先调用普通认证刷新；外部撤销失败不会把已成功的本地退出改为失败。数据库错误到状态码的共同映射以 [error.rs](../../crates/backend/src/http/error.rs) 为准。

## 输入和输出对象

下列字段均使用实际序列化名称；除单独说明外，字段必填且不能为 null。UUID 在线上为字符串，生成的新内部 ID 为 UUIDv7，但入口 UUID 解析并不限定版本。

| 对象 | 字段与结构 |
| --- | --- |
| `TextImport` | `title: string`、`occurred_at: i64`、`source: Source`、`session_id: string`、`history: string`、`idempotency_key: string` |
| `NewPath` | `session_id: string`、`history: string`、`occurred_at: i64`、`idempotency_key: string` |
| `UpdatePath` | `history: string`、`occurred_at: i64`、`idempotency_key: string` |
| `Owner` | `id: UUID`、`email: string`、`identity_id: UUID` |
| `Conversation`（详情内） | `id: UUID`、`owner_id: UUID`、`title: string`、`source: Source` |
| `Message` | `id: UUID`、`owner_id: UUID`、`conversation_id: UUID`、`parent_message_id: UUID \| null`、`role: Role`、`content: string`、`created_order: i64` |
| `Detail` | `conversation: Conversation`、`messages: Message[]`、`paths: ConversationPath[]` |
| `ConversationPath`（详情内） | `id: UUID`、`session_id: string`、`head_message_id: UUID`、`occurred_at/created_at/updated_at: i64`、`message_count: i64`、`original_link: string` |
| `Summary` | `id: UUID`、`title: string`、`source: Source`、`session_ids: string[]`、`path_count: i64`、`path_id: UUID`、`occurred_at: i64`、`head_message_id: UUID`、`message_count: i64` |
| `ImportResult` | `import_id: UUID`、`conversation_id: UUID`、`path_id: UUID`、`head_message_id: UUID`、`created: usize`、`reused: usize` |
| `Record` | `id: UUID`、`updatedAt: i64`、`isDeleted: boolean`、`body: 任意 JSON`；body 必填，允许显式 null |
| `PublishedRecord` | `ownerId: UUID`、`serverVersion: 十进制字符串`、`record: Record` |
| `UploadResult` | `{status: "accepted", record: PublishedRecord}` 或 `{status: "retained", record: PublishedRecord}` |
| `SyncPage` | `records: PublishedRecord[]`、`cursor: 十进制字符串` |

类型来源：[领域对象](../../crates/domain/src/conversation.rs)、[同步对象](../../crates/domain/src/sync.rs)、[列表](../../crates/db/src/conversation_list.rs)、[详情](../../crates/db/src/conversation_tree.rs)、[Owner](../../crates/db/src/owner.rs)、[导入结果](../../crates/db/src/import.rs)。

`Source` 严格取 `chatgpt/gemini/grok`，`Role` 严格取 `user/assistant`。Message 响应的 `parent_message_id` 总是出现，根消息为 null；不能把它生成成可省略字段。请求/响应同一 Rust 类型不一定具有同一必填语义，后续需要分别检查。`created/reused` 为本次导入新建和复用的消息数量，不是时间；前端当前只声明 ImportResult 中的三个会话/路径/head 标识。

`TextImport`、`NewPath`、`UpdatePath`、会话元数据以及 Record 外壳拒绝未知字段。同步 pull 和认证 callback 的 query 未启用 `deny_unknown_fields`，额外参数被忽略。history 内的 MessageInput 只读取 `role/content`，忽略其他字段；不得将其误标为拒绝额外字段。

## 时间、游标和开放 JSON

- `occurred_at`、路径 `created_at/updated_at`、同步 `updatedAt` 均为 Unix epoch 毫秒。`created_order`、`message_count`、`path_count` 是顺序或数量，不是时间。
- 导入 `occurred_at` 接受 `-62135596800000..=253402300799999`，在 JS 安全整数范围内。同步 `updatedAt` 接受整个有符号 i64 范围，当前没有安全整数限制；同样不能宣称所有 i64 计数都保证 JS 无损。阶段二应记录风险，不能擅自把现有 number 改成 string。
- `ServerVersion` 接受 `0..=9223372036854775807`，以规范十进制字符串传输；禁止负号、正号、前导零、空串、小数和溢出。query 的初始 cursor 为 `0`，JSON 中为 `"0"`。schema 使用 string 和数字模式，i64 上界继续由 Rust 解析检查；普通 pattern 不代表已验证数值上界。
- `limit` 先解析为 u32，再由数据库入口检查 `1..=1000`。没有省略默认值。
- `Record.body` 按现有独立记录协议保持开放，允许对象、数组、标量和 null。数据库保存 jsonb，未读取业务字段或要求对象。此接口不负责修改导入会话树；收紧结构或新增业务类型需另行设计。JSONB 和 serde_json 对深度、数值等仍有实现限制，“开放”不表示无限容量或任意精度。

依据：[同步根决策](../../specs/decisions/server/sync/0-client-timestamp-lww-and-global-seq.md)、[数据库同步实现](../../crates/db/src/sync.rs)。本阶段确认的是现行协议，不采用记忆中尚未落地的同步改造方向。

## 导入与容量

- `title`：trim 后非空，原值保留，最多 1024 UTF-8 字节。
- `session_id`：1..512 UTF-8 字节；拒绝 `.`、`..`、空白、控制字符以及 `/\\?#%:`；保持原值，来源链接单独编码。
- `idempotency_key`：trim 后非空，最多 128 UTF-8 字节，参与 Owner 范围的请求幂等。
- `history` 是包含 JSON 数组文本的字符串，而不是 HTTP JSON 内直接嵌入的数组；文件入口则接收原始字节。数组非空，元素的 role/content 必填；content 可为空且不规范化空白与换行。
- 默认原始 history 上限 8 MiB、10000 条消息、单条 content 1 MiB、JSON 嵌套深度 32；这些是可注入的 `ImportLimits`，不是所有部署固定常量。
- 业务 Router 的 HTTP body 上限为 `limits.bytes * 6 + 64 KiB`（饱和运算），默认 50,397,184 字节；它也影响同步和元数据入口。JSON 转义可能放大原文，所以原始 history 限制另外执行。
- multipart 必须包含 title、occurred_at、source、session_id、history、idempotency_key 六个字段，拒绝额外或重复字段；history 上限为 limits.bytes，其余每项最多 1024 字节。除 history 外必须是 UTF-8 文本；occurred_at 是可解析成 i64 的文本。filename 和 part 的 MIME 类型当前不是校验条件，history 可为文件或文本 part。
- 字节限制不能直接翻译成 JSON Schema `maxLength`（字符数量）。应写进 description，保留服务端字节校验及边界测试；本轮不添加隐含的字符长度限制。

## 错误与中间件边界

业务错误两种 JSON 结构：

| 结构 | 状态码与含义 |
| --- | --- |
| `{kind: "syntax" \| "field" \| "limit", path: string, message: string}` | syntax/field 为 400，limit 为 413 |
| `{error: string, login: string}` | 401 `authentication_required`（login=`/auth/login`）；403 `origin_rejected`；404 `not_found`；409 `identity_or_request_conflict` 或 `session_already_exists`；503 `identity_unavailable`；500 `persistence_failed`。其他 login 为 `""`，不省略 |

实际 extractor 行为不能统一假定为上述 JSON：

| 来源 | 当前行为 |
| --- | --- |
| UUID Path / Query 解析失败 | 原生 400 T；中间件可能先返回认证或 Origin 错误 |
| POST sync 的原生 Json extractor | 语法错误 400 T、数据类型错误 422 T、缺少/错误 Content-Type 415 T、超出 body 上限 413 T |
| 文本导入和路径创建/更新 | 捕获 JsonRejection：body 过大转 413 J，其他转 400 J，path=`request` |
| PUT 会话元数据 | 所有 JsonRejection 都转 400 J，包括 body 过大；这是现状差异，不在盘点阶段修正 |
| multipart 初始 boundary 无效 | 原生 400 T |
| multipart 读取 | `next_field` 错误映射 400 J；`field.chunk` 错误映射 413 J；显式字段容量错误 413 J，所以不是所有超限都保证 413 |
| 未匹配路由/不支持方法 | 与应用静态资源 fallback 和 Axum 默认响应有关；后续契约测试单独判断，不能伪装为 ApiError |

原生行为核对了锁定的 Axum 0.8.9 源码 `extract/rejection.rs`、`extract/multipart.rs`。后续若决定统一错误，需要显式改变 handler/extractor 行为并补测试；不能只把 OpenAPI 写成理想化的统一 JSON。

## 第二阶段建模边界

详情当前输出前端未声明的 `owner_id`、`conversation_id`，`/api/me` 输出 `identity_id`；这些是现有协议字段，不属于凭证，但应在整理 DTO 时逐项确认是否保留。第一阶段不移除字段。前端当前能够读取 JSON 错误中的 path/message 或 error，却丢弃原生纯文本错误；这也是迁移时需要显式处理的行为。

本文件是实现盘点，不新增领域决策或承诺。后续生成的 OpenAPI 成为字段结构的事实来源后，应收缩这里重复的字段表，保留无法由 schema 表达的语义与风险说明。
