# 线性对话路径导入核心测试用例

本文跟踪[用户提交线性路径导入根决策](../../../decisions/server/import/0-user-submitted-linear-path.md)中输入兼容性、原子性、幂等和并发合并风险。实现证据随对应提交维护；没有直接验证的义务继续标记为 `Missing`。

## Text and file inputs must have identical parsing semantics

### 风险

相同 JSON 因粘贴或文件入口不同而产生不同消息、字段兼容范围或错误结果。

### 前置状态

准备一份包含连续 user、以 user 结束及额外字段的合法 JSON，并准备 role 错误、content 非字符串和空数组等非法变体。

### 触发

分别通过文本框和 JSON 文件提交每个输入。

### 必须成立

两种入口对合法输入生成完全相同的 Message 序列，只保留 role/content；对每个非法输入返回相同错误类别和数组位置。

### 禁止结果

不得因入口不同默认角色、跳过元素、合并消息、接受不同字段类型或改变 content。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 两种入口共用相同合法输入语义 | Covered | `crates/backend/tests/http.rs::authenticated_http_imports_preserve_scope_and_file_parity`（真实 HTTP router + PG，ignore） |
| 两种入口共用相同非法输入与错误定位语义 | Covered | `crates/backend/tests/http.rs::authenticated_http_imports_preserve_scope_and_file_parity`（字段与容量错误，真实 PG，ignore） |
| 额外字段被忽略且不持久化 | Covered | `palace-domain::import::tests::preserves_linear_input_exactly` 与 `crates/backend/tests/http.rs::authenticated_http_imports_preserve_scope_and_file_parity`；后者断言持久化后完整响应对象不含额外字段 |
| 原始 Markdown、空白和换行不被规范化 | Covered | `palace-domain::import::tests::preserves_linear_input_exactly`、`accepts_all_source_export_samples`；`crates/backend/tests/http.rs::authenticated_http_imports_preserve_scope_and_file_parity`（真实 PG，ignore） |

### 决策依据

根决策 D1 及不变量 2、3、5。

## A failed import must leave no partial business state

### 风险

解析、限制校验或中途数据库失败后留下空 Conversation、部分 Message 或成功 Import，用户误以为历史完整。

### 前置状态

分别准备字段不合法、容量超限和在写入第 N 条消息时失败的请求；记录提交前数据库状态。

### 触发

执行每次导入并观察响应和持久化状态。

### 必须成立

请求返回可区分的失败类别；Conversation、Message 与 conversation_import 均回到提交前状态。超限在业务写入前失败。

### 禁止结果

不得跳过坏消息后返回成功，不得留下无导入记录的部分路径，也不得留下指向未提交 Message 的 head。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 字段校验失败不产生业务写入 | Covered | `crates/backend/tests/http.rs::authenticated_http_imports_preserve_scope_and_file_parity`（失败后断言三张业务表计数） |
| 容量超限在业务写入前失败 | Covered | `palace-domain::import::tests::reports_position_and_limits_before_writes`；`crates/backend/tests/http.rs::authenticated_http_imports_preserve_scope_and_file_parity` |
| 任意持久化步骤失败使三类记录共同回滚 | Covered | `crates/db/tests/postgres.rs::concurrent_imports_reuse_prefix_and_failures_roll_back`，真实 PostgreSQL 17，默认 ignore；身份输入为测试提供，不含 OIDC 协议验证 |
| 错误类别与数组下标可定位 | Covered | `palace-domain` 单元测试 `reports_position_and_limits_before_writes`；持久化及 HTTP 证据另列 |

### 决策依据

根决策 D3、D5 及不变量 4。

## Repeated and concurrent imports must converge on one exact path structure

### 风险

重试或并发导入创建重复 Conversation、重复前缀节点或多份相同后缀，使消息树取决于请求时序。

### 前置状态

同一 Owner Scope 准备相同 source/session_id 的 `A-B-C` 请求、不同幂等键的重复请求及 `A-B-D` 请求。

### 触发

先重复导入完整路径，再并发导入共享前缀的两条路径，并模拟客户端超时后用同一幂等键重试。

### 必须成立

同一来源会话只有一个 Conversation；完全相同路径不重复创建 Message；两条不同路径只在 B 后产生 C、D；相同幂等请求返回原结果。并发冲突通过串行处理或唯一约束后的重试收敛。

### 禁止结果

不得出现两个相同来源 Conversation、内容相同的重复前缀兄弟或不同请求共用同一幂等键却静默成功。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 完全重复导入复用已有路径 | Covered | `crates/db/tests/postgres.rs::concurrent_imports_reuse_prefix_and_failures_roll_back`，真实 PostgreSQL 17，默认 ignore；身份输入为测试提供，不含 OIDC 协议验证 |
| `A-B-C` 与 `A-B-D` 并发后只共享一份 A、B | Covered | `crates/db/tests/postgres.rs::concurrent_imports_reuse_prefix_and_failures_roll_back`，真实 PostgreSQL 17，默认 ignore；身份输入为测试提供，不含 OIDC 协议验证 |
| 同一幂等键的相同请求返回已提交结果 | Covered | `crates/db/tests/postgres.rs::concurrent_imports_reuse_prefix_and_failures_roll_back`，真实 PostgreSQL 17，默认 ignore；身份输入为测试提供，不含 OIDC 协议验证 |
| 同一幂等键被不同请求复用时失败 | Covered | `crates/db/tests/postgres.rs::concurrent_imports_reuse_prefix_and_failures_roll_back`，真实 PostgreSQL 17，默认 ignore；身份输入为测试提供，不含 OIDC 协议验证 |

### 决策依据

根决策 D3、D4、D5 及不变量 4、6、7。

## Import must not fetch source pages and rendered Markdown must remain inert

### 风险

导入请求触发服务端访问不可信来源地址，或保存的 Markdown 在展示时执行脚本和事件处理器。

### 前置状态

准备不可访问的原始会话、带外部地址的 session_id，以及包含原始 HTML、脚本和事件属性的 Markdown content。

### 触发

提交导入并在支持 Markdown 的界面读取结果。

### 必须成立

导入是否成功只由提交数据决定，不产生来源页面网络请求；数据库保留原始字符串，展示结果经过安全渲染且危险内容不执行。

### 禁止结果

不得以跳转模板为抓取地址，不得因来源页面不可访问拒绝合法 JSON，不得直接把 content 作为可信 HTML 执行。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 导入流程不访问 Web Chat 页面 | Covered | `crates/backend/tests/http.rs::authenticated_http_imports_preserve_scope_and_file_parity`；配置可观察的失败来源端点，完整导入/读取后访问计数为 0 |
| 来源页面状态不影响合法 JSON 导入 | Covered | `crates/backend/tests/http.rs::authenticated_http_imports_preserve_scope_and_file_parity`；来源端点设为 503，导入结果不受影响 |
| 存储保留原文且渲染阻止活动内容执行 | Partial | `crates/backend/tests/http.rs::authenticated_http_imports_preserve_scope_and_file_parity` 验证 script 原文和 application/json；本次 server 不提供 Markdown UI，渲染测试留给展示端 |

### 决策依据

根决策 D2、D3 及不变量 1、5。
