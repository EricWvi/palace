# 前后端 JSON 接口契约实施计划

日期：2026-09-20（Asia/Shanghai）

状态：第一、第二阶段已实施，第三、第四阶段尚未开始。HTTP DTO 和离线 OpenAPI 契约已落地，既有生产接口语义保持不变。

## 目标与方案

以 Rust API 请求、响应类型和路由声明为唯一维护来源，通过 `utoipa` 生成 OpenAPI 3.1，通过 `openapi-typescript` 生成 TypeScript 类型，使用 `openapi-fetch` 建立类型化调用。CI 检查生成产物是否同步，并验证真实 HTTP 响应符合契约。

最终应实现：后端修改接口字段后，前端不匹配的使用能够在类型检查中失败；实际响应与声明不符时，契约测试失败；接口文档从同一契约生成。

```text
Rust API DTO + 路由声明
           ↓ utoipa
      OpenAPI 3.1
       ├── 接口参考文档
       ├── HTTP 契约测试
       └── openapi-typescript → TS 类型 → openapi-fetch
```

生成描述不等于执行校验。服务端继续负责反序列化、值域限制和领域规则；权限、归属、幂等和事务约束由业务代码及集成测试保障。

## 当前缺口

| 位置 | 当前行为 | 需要补齐的约束 |
| --- | --- | --- |
| `apps/palace-web/src/lib/api.ts` | 手写 DTO，使用 `response.json() as Promise<T>` | 请求路径与参数、响应类型由契约推导，避免调用方自行指定返回类型 |
| `crates/backend/src/http/business.rs` | 请求已有反序列化和领域校验，部分响应用 `json!` 拼装 | 公开请求与响应使用明确的 API DTO |
| `crates/backend/src/http/error.rs` | 输入错误和其他错误返回不同结构 | 将实际错误分支、状态码和媒体类型纳入契约 |
| `crates/domain/src/sync.rs` | `ServerVersion` 序列化为字符串，`Record.body` 允许任意 JSON | 精确描述传输格式，明确开放 JSON 的业务边界 |
| `Taskfile.yml` | 尚无前后端接口契约生成与检查任务 | 可复现生成、漂移检查、HTTP 契约验证 |

## 范围与约束

- 覆盖现有 `/api/*` 的全部方法，以及前端使用的 `/auth/*` 接口；认证跳转、回调和非 JSON 响应按实际 HTTP 行为描述。
- 覆盖路径与查询参数、JSON 请求、multipart 文件导入、成功响应、错误响应和认证要求。
- 保留当前业务语义。发现实际行为与预期不一致时，显式记录并决定修正实现还是契约，不以生成类型掩盖差异。
- API DTO 归属于 HTTP 边界，避免把数据库内部模型及敏感字段直接变成公开协议；优先使用私有模块和显式导出，不为本次工作强行新增 crate。
- 不在本计划中改造同步冲突算法，也不借契约建设引入新的多记录同步协议。
- 本轮不默认增加浏览器生产环境的全量响应校验。先建立生成与 CI 验证闭环；后续若需要运行时校验，从同一契约派生。
- 不维护旧的手写类型或旧请求封装作为兼容层，迁移完成后删除无用代码。
- 不在本地构建 Docker 镜像，不运行 buildkit 容器；容器测试使用已有镜像。

## 第一阶段：盘点接口并确定传输语义

- [x] 从实际 Router 和前端调用点整理接口清单，逐项记录方法、路径、输入、响应、状态码、媒体类型与鉴权要求。
- [x] 检查 extractor、请求体大小限制和认证中间件产生的错误，避免只记录 handler 内的 `ApiError`。
- [x] 明确字段缺失与 `null`、枚举编码、未知字段策略、时间单位和整数范围。
- [x] 固定 `ServerVersion` 的十进制字符串语义，验证生成结果不会错误变成 JavaScript `number`。
- [x] 确认 `Record.body` 是否有意开放：若开放，在契约中明确；若要按业务类型约束，先确定类型及规则，不在本轮凭空补造领域协议。
- [x] 核对长度限制的计量单位；例如当前标题限制按字节计算，不能直接用 JSON Schema 的字符长度宣称等价。
- [x] 验证 `utoipa`、OpenAPI 3.1、TS 生成器和测试验证器的兼容性，选定版本并锁定依赖；用可空字段、带标签枚举、字符串游标和 multipart 做最小验证。
- [x] 确定契约和 TS 生成文件的仓库路径，以及无需数据库、OIDC 服务即可执行的导出入口。

验收：接口清单覆盖实际路由与前端调用；每项特殊传输语义有明确结论；工具链可以处理项目现有协议。

第一阶段交付与证据：

- [接口盘点](../../docs/api/inventory.md)：17 个显式路由操作、前端调用点、错误和媒体类型、字段及容量语义。
- [工具链结论](../../docs/api/toolchain.md)：固定版本、TS 5/6 隔离、特殊序列化与 multipart 验证、后续产物路径和离线入口。
- `task test:api-toolchain`：最小组合验证；不依赖 PostgreSQL、OIDC 或容器，不等同于生产 HTTP 契约测试。
- `Record.body` 保持当前开放 JSON，且必填、允许 null；未引入新的业务结构或同步协议。
- 正式契约选择 `contracts/openapi.json`，前端类型选择 `apps/palace-web/src/lib/generated/api.ts`；第二阶段实现离线 `export_openapi` example。

## 第二阶段：建立 Rust 契约来源和生成流程

- [x] 在 HTTP 边界整理请求、成功响应和错误 DTO，以明确类型替代公开响应的临时 JSON 拼装。
- [x] 明确 DTO 与领域、数据库类型的转换，保持业务校验的所有权，防止认证内部状态泄漏。
- [x] 为 DTO 和路由添加契约声明，覆盖第一阶段清单；尽可能复用路由注册信息，减少运行路由和文档各自维护的偏差。
- [x] 生成 OpenAPI 3.1 文件，确保描述的是 Serde 的实际输出，而非 Rust 内存表示。
- [x] 对照 schema 中的必填、枚举、未知字段和值域约束检查服务端执行路径；无法用标准 schema 精确表达的规则写入说明并保留行为测试。
- [x] 新增契约导出任务，要求输出顺序稳定，不混入当前时间、机器路径或服务状态。

验收：连续生成两次得到相同产物；现有接口都在契约内；自定义序列化与错误分支符合实际响应；仅添加描述不会被误认为已增加运行时校验。

第二阶段交付与证据：

- [正式 OpenAPI](../../contracts/openapi.json)：覆盖 17 个显式操作、26 个 schema、请求/响应、错误媒体类型、认证和响应头。
- [HTTP DTO](../../crates/backend/src/http/dto.rs)：公开字段显式列举，替代响应 JSON 拼装；领域和数据库继续执行原有校验。
- `api_contract()` 与 `examples/export_openapi.rs`：离线生成，`task api:export` 写入版本化契约；两次导出逐字节相等，并与提交产物一致。
- `task test:api-schema`：schema、引用和关键正反例验证；`task test:api-toolchain`：TS 与 multipart 工具组合验证。
- `task test`、`task test:integration` 通过；后者包含 18 个真实 PostgreSQL 用例和 2 个真实 HTTP 用例。
- Router 与 handler 契约复用路径常量；方法及操作注册仍各自维护，第四阶段继续补自动覆盖门禁。前端调用未迁移，尚未将 Node 契约检查加入默认 `task test`。

## 第三阶段：迁移前端调用并建立接口文档

- [ ] 从 OpenAPI 生成 TypeScript 类型，标记生成文件禁止手工修改。
- [ ] 接入 `openapi-fetch`，由方法、路径和参数推导请求与响应类型。
- [ ] 迁移 `api.ts` 及所有调用点，保留 cookie 携带、错误文案、React Query 行为和文件上传流程。
- [ ] 删除重复的手写 API DTO 与任意返回类型断言；UI 专用状态模型仍由前端维护。
- [ ] 验证 multipart 的浏览器编码与请求封装，不以 JSON 请求的默认行为替代文件上传语义。
- [ ] 在 `docs/` 更新接口参考文档入口、生成方式、字段语义和错误处理说明；字段表由契约提供，业务约束由说明文档解释。

验收：所有范围内的前端调用来自生成类型；错误字段或参数能够被 TypeScript 检出；列表、详情、导入、编辑、删除及认证相关行为通过对应回归验证。

## 第四阶段：建立 CI 契约验证和验收闭环

- [ ] 新增生成检查任务：在干净检出中重新生成契约与前端类型，存在修改或遗漏的生成文件时失败；检查模式不覆盖开发者本地改动。
- [ ] 增加真实 Router 的 HTTP 契约测试，按操作、状态码和媒体类型选择响应 schema，验证真实响应体，不仅验证手写 mock。
- [ ] 对关键输入增加反例验证，证明服务端拒绝不符合声明的字段和类型；涉及数据库状态的规则使用真实 PostgreSQL 集成测试。
- [ ] 覆盖缺失与 `null`、未知字段、枚举、自定义序列化、超出 JS 安全整数范围的游标、输入错误和认证错误等高风险边界。
- [ ] 检查路由与 OpenAPI 操作的覆盖关系，防止新增接口遗漏契约；具体采用共享注册还是显式覆盖检查，在实现时结合 Router 能力选择。
- [ ] 将无需容器的生成和契约检查纳入默认检查；将需要 PostgreSQL 的契约用例纳入 Taskfile 显式集成测试任务；不新增 GitHub Actions 测试作业。
- [ ] 保留现有 `task test:contract` 的 Authelia 协议测试语义，为 API schema 检查使用不同任务名。
- [ ] 对照 `specs` 中相关 ADR 检查是否改变系统承诺；需要新增或修改 ADR 时按现有生命周期推进，只有状态为 `approved` 后才维护对应核心测试用例。

Eric 备注：github action 只做构建，CI 放在 taskfile

验收：过期生成文件、真实响应偏离 schema、范围内路由遗漏契约，均有能直接失败的检查；默认检查与容器检查的覆盖边界明确。

## 验证与完成条件

实施时先运行 `task --list` 确认任务，并按阶段运行最小相关检查。修改代码后执行 `task format`；前端迁移运行前端 lint 和测试，Rust 变更运行相关 crate 测试。仓库范围改造完成前运行 `task test`，并显式执行本次新增或受影响的 HTTP/PostgreSQL 集成测试。

`task test` 默认忽略容器测试，不能以它通过替代集成验证。缺少已有镜像等环境条件时如实记录未验证项，不将其标为完成。前端测试继续遵守 clean-stderr 和异步 `act` 边界要求。

最终交付物：

- Rust API DTO、路由契约声明和确定性的导出入口。
- 纳入版本管理的 OpenAPI 与前端生成类型，以及完成迁移的客户端调用。
- 契约生成、漂移检查、真实 HTTP 验证任务及 Taskfile CI 接入。
- 更新后的接口文档和必要的 ADR、核心用例证据。

## 选择依据与后续边界

采用代码优先是因为当前服务端已有 Rust 类型和校验逻辑，能降低双份维护成本。若未来由多个独立团队共同制定协议，可重新评估先写 OpenAPI 再生成实现的方案。

`schemars` 适合独立配置或导入 JSON 的 schema，但单独使用它无法覆盖 HTTP 操作；本轮不并行维护另一条 schema 生成链。若未来引入 Ajv 等运行时验证器，需要确认方言、引用解析与 format 行为，并复用当前契约。

官方依据：

- [JSON Schema 对象约束](https://json-schema.org/understanding-json-schema/reference/object)：`required`、额外字段和字段缺失的语义。
- [OpenAPI 3.1 规范](https://spec.openapis.org/oas/v3.1.0)：HTTP 契约与基于 JSON Schema 2020-12 的 Schema Object。
- [utoipa](https://docs.rs/utoipa/latest/utoipa/) 与 [ToSchema](https://docs.rs/utoipa/latest/utoipa/derive.ToSchema.html)：Rust 契约生成及 Serde 支持边界。
- [openapi-typescript](https://openapi-ts.dev/introduction) 与 [openapi-fetch](https://openapi-ts.dev/openapi-fetch/)：类型生成及类型化 HTTP 调用，不等同于响应运行时验证。
- [schemars](https://docs.rs/schemars/latest/schemars/) 与 [Ajv 方言支持](https://ajv.js.org/json-schema.html)：独立 JSON schema 和后续运行时校验的备选工具。
