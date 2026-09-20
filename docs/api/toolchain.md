# API 契约工具链与第一阶段结论

第一阶段选择 Rust 类型及路由声明 → OpenAPI 3.1 → TypeScript 的生成方向。最小样例已落地为 `task test:api-toolchain`；它验证工具组合及特殊传输语义，不代表生产路由已经接入 OpenAPI，也不替代真实 HTTP/PostgreSQL 契约测试。

## 固定版本与职责

| 依赖 | 版本 | 当前用途 |
| --- | --- | --- |
| utoipa / utoipa-gen | 5.5.0 | backend 的开发依赖，从样例类型和路由生成 OpenAPI 3.1.0 |
| openapi-typescript | 7.13.0 | 根开发依赖，生成 TS 类型 |
| TypeScript（生成工具） | 5.9.3 | 根开发依赖，满足生成器声明的 `^5.x` peer dependency |
| TypeScript（Web） | 6.0.3（当前 lock） | 保留 Web 的 `^6.0.3` 声明；验证生成结果兼容应用编译器 |
| openapi-fetch | 0.17.0 | 根开发依赖，用于 multipart 客户端验证；第三阶段接入应用时移入 Web 运行依赖 |
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

当前提取器只处理 fixture 中的本地 schema 引用，不是支持外部引用和所有 OpenAPI 方言的通用验证框架；第四阶段接入真实契约时必须按实际 schema 扩展和检查。schema 标注不会自动增加运行时约束。

## 确定的第二阶段路径与入口

以下是后续实施约定，目前未创建生产契约和导出入口：

| 产物或职责 | 路径 / 命令 |
| --- | --- |
| Rust HTTP DTO 与契约组装 | `crates/backend/src/http/` 下独立私有模块，模块根使用 `name.rs`；由 backend 显式导出最小契约生成 API |
| 离线导出入口 | `crates/backend/examples/export_openapi.rs`；`cargo run --locked -p palace-backend --example export_openapi` 输出 JSON 到 stdout |
| 正式契约 | `contracts/openapi.json`，提交版本管理 |
| TS 生成类型 | `apps/palace-web/src/lib/generated/api.ts`，提交版本管理，禁止手改 |
| 调用封装 | `apps/palace-web/src/lib/api.ts`，保留业务错误文案及凭证策略 |
| 接口说明 | `docs/api/`，文档导航由 `docs/README.md` 提供 |
| 后续 Taskfile 入口 | `api:export`、`api:generate`、`api:check`；实现时分别负责导出、完整生成和不覆盖本地文件的漂移检查 |

离线导出不得创建 Server、连接 PostgreSQL 或联系 OIDC；契约信息采用固定 metadata，避免时间戳、环境路径或部署配置使生成结果漂移。当前 `contract_probe` 只服务于第一阶段验证，不能作为实际 API 的导出入口。

## CI 与后续工作

遵循 Eric 的备注：GitHub Actions 只做构建，CI 放在 Taskfile。本阶段新增独立 `test:api-toolchain` 任务；默认 `task test` 尚不包含它。第四阶段将生产契约生成检查和无容器测试接入默认任务，需要 PostgreSQL 的测试接入显式集成任务；不添加 GitHub Actions 测试作业，不改变现有 `test:contract` 的 Authelia 用途。

目前没有改变生产接口行为，也没有创建新领域 ADR。原生纯文本错误、元数据超限映射为 400、详情额外字段等现状见 [接口盘点](inventory.md)；需要改变这些行为时显式设计并同步相应证据，不能通过修改 schema 隐藏差异。

2026-09-20 验证结果：`task test:api-toolchain`、`task format` 和 `task test` 通过。全量任务包含前端 lint、22 个前端测试及 Rust workspace lint/默认测试；默认忽略的容器集成测试本阶段未运行。第一阶段未修改生产接口或数据库行为，也没有把这些容器用例记为本次契约验证证据。

官方依据：[utoipa ToSchema](https://docs.rs/utoipa/5.5.0/utoipa/derive.ToSchema.html)、[openapi-typescript Node API](https://openapi-ts.dev/node)、[openapi-fetch](https://openapi-ts.dev/openapi-fetch/)、[Ajv JSON Schema 方言](https://ajv.js.org/json-schema.html)。具体版本及适配选择以本仓库锁文件和可执行样例为证据。
