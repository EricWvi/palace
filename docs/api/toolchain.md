# API 契约工具链与导出流程

Rust HTTP DTO 及实际 handler 的声明生成 OpenAPI 3.1。已覆盖 17 个显式路由操作，前端通过生成类型与 openapi-fetch 调用这些接口。`task test:api-toolchain` 仍验证独立最小样例，`task test:api-schema` 验证正式契约；两者都不替代真实 HTTP/PostgreSQL 契约测试。

## 固定版本与职责

| 依赖 | 版本 | 当前用途 |
| --- | --- | --- |
| utoipa / utoipa-gen | 5.5.0 | backend 的运行依赖，从实际 DTO 与 handler 声明生成 OpenAPI 3.1.0 |
| openapi-typescript | 7.13.0 | 根开发依赖，生成 TS 类型 |
| TypeScript（生成工具） | 5.9.3 | 根开发依赖，满足生成器声明的 `^5.x` peer dependency |
| TypeScript（Web） | 6.0.3（当前 lock） | 保留 Web 的 `^6.0.3` 声明；验证生成结果兼容应用编译器 |
| openapi-fetch | 0.17.0 | Web 运行依赖，提供按方法、路径和参数推导类型的客户端 |
| Ajv | 8.20.0 | 使用 `ajv/dist/2020.js`，验证提取后的 JSON Schema 2020-12 |
| ajv-formats | 3.0.1 | UUID 等格式验证 |

新增工具直接依赖使用精确版本，传递依赖由 `Cargo.lock`、`package-lock.json` 锁定。工具 TS 5 与 Web TS 6 分开解析；不使用 `--force` 或 `--legacy-peer-deps` 绕开 peer 要求，不降低 Web 版本。Node 沿用项目要求的 24+。

## 可执行验证

```bash
task test:api-toolchain
```

任务调用 [验证脚本](../../scripts/check-contract-toolchain.mjs)，执行以下步骤：

1. `cargo run --locked --quiet -p palace-backend --example contract_probe` 在内存中生成 fixture 契约和 Serde 样本，不启动服务、不读取数据库或 OIDC 配置。
2. 用真实 `ServerVersion` 序列化超过 JS 安全整数范围的游标，验证结果仍是精确字符串；检查非法格式和超过 i64 上界的字符串被 Rust 拒绝。
3. 使用真实 Record 反序列化确认 body 必填且允许显式 null；用中文标题验证字节上限与字符长度不等价。
4. Ajv 验证生成的 schema 接受合法结果、拒绝数字游标、非规范游标、缺失 nullable 字段、非法 UUID、额外字段和错误枚举分支。
5. 从样例 OpenAPI 生成 TS，通过工具 TS 5 和 Web 实际 TS 6 编译器检查正例与 `@ts-expect-error` 反例。
6. 类型化客户端发送 multipart 到内存中的 fetch 实现，验证边界头由浏览器兼容 Request 自动生成、File 内容与五个文本字段保持完整、凭证模式保留，以及响应类型能够按 status 收窄。

脚本只在仓库忽略的 `.tmp/contract-probe-*` 中写入中间文件，成功或失败都会清理。没有网络 HTTP 请求；npm/Cargo 依赖需预先安装或允许包管理器下载。验证用例无需容器。

## 已确认的工具适配

| 问题 | 结论 |
| --- | --- |
| 自定义 Serde 字符串游标 | schema 显式覆盖为 `String`，不从内部 i64 推导。数字正则只检查词法，上界由 Rust 保证 |
| 响应中的 `Option<Uuid>` | utoipa 默认将 Option 视为可省略；当前响应总输出 null 或 UUID，需 `#[schema(required = true)]` |
| 带标签枚举 | `serde(tag = "status", content = "record", rename_all = "snake_case")` 可生成可验证、可收窄的联合类型 |
| 严格对象 | `serde(deny_unknown_fields)` 可生成拒绝额外字段的 schema；仅在实际严格的对象上使用 |
| multipart 文件 | OpenAPI 使用 string/binary 描述文件 part；TS 生成时通过 transform 映射为 Blob，接受浏览器 File；客户端显式使用 FormData serializer |
| Ajv 输入 | 提取 components.schemas，将本地 `#/components/schemas/` 引用映射到 `#/$defs/`；不把整个 OpenAPI 文档当成 JSON Schema |
| binary format | Ajv 将其注册为非校验注解，文件编码由 multipart Request 测试验证；不声称 Ajv 校验了二进制传输 |
| 非标准业务限制 | UTF-8 字节数、trim 后非空、游标数值上界、资源归属与事务约束仍由服务端执行 |

当前两个 schema 验证脚本只处理本地 components 引用，不是支持外部引用和所有 OpenAPI 方言的通用验证框架；第四阶段验证真实 HTTP 响应时需要选择正确操作、状态码和媒体类型，并继续覆盖这些解析边界。schema 标注不会自动增加运行时约束。

## 第二阶段导出与校验

```bash
task api:export
task test:api-schema
```

`api:export` 编译并运行离线 example，成功拿到完整 JSON 后才写入 `contracts/openapi.json`。`test:api-schema` 验证导出的 26 个 schema、所有本地引用及关键正反例，并确认选定的 TS 生成器能读取正式契约。它不会偷偷重写契约，检查生成漂移的默认门禁留在第四阶段。

声明位于实际 handler 的 `#[utoipa::path]`；路径常量同时供 Router 使用。方法注册和 OpenAPI 的操作列表仍需分别维护，目前有 17 操作清单测试，尚不宣称已具备自动路由覆盖检查。

DTO 仅公开明确列举的字段。DB/domain 到 DTO 的转换按完整 JSON 对象比较，检验 null、字符串游标、开放 body 与所有公开标识保持一致；错误 responder 测试检查状态码、媒体类型和整个 JSON 对象。业务校验仍在原 parser、domain 和数据库事务中执行。

| 产物或职责 | 路径 / 命令 |
| --- | --- |
| Rust HTTP DTO 与契约组装 | `crates/backend/src/http/` 下独立私有模块，模块根使用 `name.rs`；由 backend 显式导出 `api_contract()` |
| 离线导出入口 | `crates/backend/examples/export_openapi.rs`；`cargo run --locked -p palace-backend --example export_openapi` 输出 JSON 到 stdout |
| 正式契约 | `contracts/openapi.json`，提交版本管理 |
| TS 生成类型 | `apps/palace-web/src/lib/generated/api.ts`，提交版本管理，禁止手改 |
| 调用封装 | `apps/palace-web/src/lib/api.ts`，保留业务错误文案及凭证策略 |
| 接口说明 | `docs/api/`，文档导航由 `docs/README.md` 提供 |
| Taskfile 入口 | `api:export` 导出；`api:generate` 完整生成；不覆盖本地文件的 `api:check` 留在第四阶段 |

离线导出不创建 Server、不连接 PostgreSQL 或联系 OIDC；契约信息采用固定 metadata，不包含时间戳、环境路径或部署配置。当前 `contract_probe` 只服务于第一阶段验证，不能作为实际 API 的导出入口。

## 前端生成与调用

```bash
task api:generate
task lint:frontend
task test:frontend
PALACE_BROWSER_TEST_PORT=5174 task test:browser
```

`api:generate` 先离线导出正式 OpenAPI，再由 [生成脚本](../../scripts/generate-api-types.mjs) 写入 [前端类型](../../apps/palace-web/src/lib/generated/api.ts)。两份产物都提交版本管理；生成文件有禁止手改标记。修改接口时先修改 Rust DTO/handler 声明，再重新生成并迁移调用方；字段、方法或参数不匹配由前端 TypeScript 检出。

生成器只对浏览器类型视图做两项适配，原始契约保持完整：移除浏览器自动发送、应用不能手工设置的 `Origin` 请求头参数；将 `string/binary` 映射为 `Blob`，因此接受 `File`。服务端仍校验写操作的 Origin，浏览器回归验证实际上传请求携带同源 Origin。

调用使用 `apiData(api.GET("/api/conversations"))` 一类表达式。路径参数通过 `params.path` 传入并编码；JSON body 由客户端序列化。组件需要的 DTO 别名直接引用 `components["schemas"]`，不再维护手写字段表或 `request<T>` 返回断言。客户端显式使用 `credentials: "same-origin"`，React Query 的 query key、失效刷新和重试逻辑保留。

文件导入传入完整的类型化 body，再由 `serializeImport` 转为 FormData；客户端不预设 Content-Type，浏览器生成 multipart boundary。发生时间在 multipart 中为字符串，在 JSON 分支请求中为数字；重试复用原有幂等 key。

`apiData` 统一解包成功响应并抛出 `ApiError`：保留 401/403/404/409/413 的中文提示、Session ID 冲突提示及结构化字段错误。原生纯文本、代理 HTML、缺失错误字段走状态码或通用提示；网络异常原样交给 UI。空的成功响应视为不可用；当前 Web 调用均要求 JSON 成功体。登录仍通过 `/auth/login` 页面跳转。

生成类型不执行响应运行时校验，也不会强制 UUID 格式、字节限制、整数范围或数据库约束。字段及媒体类型以 [OpenAPI](../../contracts/openapi.json) 为准，特殊语义见 [接口盘点](inventory.md)。`test:browser` 使用真实 Chromium 和 mock HTTP 响应，验证实际浏览器编码与 UI 行为；它不是后端 HTTP schema 测试。

## CI 与后续工作

遵循 Eric 的备注：GitHub Actions 只做构建，CI 放在 Taskfile。`api:export`、`test:api-schema`、`test:api-toolchain` 均为独立任务；默认 `task test` 尚未纳入契约导出或 Node schema 校验，但包含新增的 Rust DTO/契约测试。第四阶段将生产契约生成检查和无容器测试接入默认任务，需要 PostgreSQL 的测试接入显式集成任务；不添加 GitHub Actions 测试作业，不改变现有 `test:contract` 的 Authelia 用途。

本次是保持生产接口行为的 HTTP DTO 重构，没有创建新领域 ADR。原生纯文本错误、元数据超限映射为 400、详情额外字段等现状见 [接口盘点](inventory.md)；需要改变这些行为时显式设计并同步相应证据，不能通过修改 schema 隐藏差异。

2026-09-20 第二阶段验证：`task format`、`task test`、`task test:api-schema`、`task test:api-toolchain` 和 `task test:integration` 通过。集成任务显式执行 18 个真实 PostgreSQL 用例及 2 个真实 HTTP 用例，使用本地已有镜像；两次离线导出逐字节一致且匹配版本化产物。本阶段未运行真实 Authelia 协议测试；没有修改 OIDC 交互逻辑。全量 HTTP 响应按 schema 验证以及漂移门禁仍属于第四阶段。

2026-09-20 第三阶段验证：`task format`、`task test`（34 个前端测试）、`task build:frontend`、`task test:api-schema`、`task test:api-toolchain` 和 `task test:browser`（2 个 Chromium 用例）通过；两次完整生成逐字节一致。前端构建保留 Vite 的大于 500 kB chunk 提示。未变更后端协议，本阶段未重跑 PostgreSQL 或 Authelia 集成测试。

官方依据：[utoipa ToSchema](https://docs.rs/utoipa/5.5.0/utoipa/derive.ToSchema.html)、[openapi-typescript Node API](https://openapi-ts.dev/node)、[openapi-fetch](https://openapi-ts.dev/openapi-fetch/)、[Ajv JSON Schema 方言](https://ajv.js.org/json-schema.html)。具体版本及适配选择以本仓库锁文件和可执行样例为证据。
