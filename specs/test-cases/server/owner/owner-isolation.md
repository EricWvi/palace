# Owner 身份与数据隔离核心测试用例

本文跟踪[Owner 与数据隔离根决策](../../../decisions/server/owner/0-authelia-identity-and-owner-scoped-records.md)及[长期 Palace Session 后续决策](../../../decisions/server/owner/20260908-persistent-revocable-owner-sessions.md)中认证绑定、email 映射、数据归属、会话和同步隔离的长期风险。实现证据随对应提交维护；没有直接验证的义务继续标记为 `Missing`。

## A verified Authelia identity must resolve to one stable owner

### 风险

同一 Authelia Identity 在重复登录或 email 变化后生成多个 Owner，或者未验证的 OIDC 数据建立 Owner Scope。

### 前置状态

准备一个有效的 `(issuer, subject, email)` OIDC 认证结果，以及签名、issuer、audience、有效期、state 或 nonce 不合法的结果；有效身份完成首次登录后在 Authelia 修改 email。

### 触发

首次登录、重复登录、email 变化后登录，并分别提交各类无效认证结果。

### 必须成立

首次有效登录原子创建一个 Owner 与一个 Owner Identity；相同 `(issuer, subject)` 始终返回同一 owner_id。email 变化只更新当前映射，不改变业务归属；任一协议校验失败都不能建立 Owner Scope。

### 禁止结果

不得按 email 创建第二个 Owner，不得把 email 或 subject 直接作为业务 owner_id，不得信任浏览器提交的身份字段。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 首次有效登录原子创建唯一 Owner/Identity | Partial | `crates/db/tests/postgres.rs::identity_and_database_constraints_isolate_owners`，真实 PostgreSQL 17，默认 ignore；身份输入为测试提供，不含 OIDC 协议验证 |
| 重复登录及 email 变化保持 owner_id 稳定 | Covered | `crates/db/tests/postgres.rs::identity_and_database_constraints_isolate_owners`，真实 PostgreSQL 17，默认 ignore；身份输入为测试提供，不含 OIDC 协议验证 |
| OIDC 任一必要校验失败都不建立 Owner Scope | Missing | 尚无实现测试 |

### 决策依据

根决策 D1、D2 及不变量 1、2、4。

## Every login must include a currently verified usable email

### 风险

已绑定 Owner 在本次认证缺少 email 时沿用数据库历史 email，掩盖 Authelia claim 配置错误，并建立没有当前 email 证明的 Owner Scope。

### 前置状态

一个 `(issuer, subject)` 已绑定 Owner 且 Palace 保存了上次 email；准备本次缺少 email、email 为空或格式不合法的有效 OIDC 身份结果。

### 触发

使用每种缺少可用 email 的结果登录，然后恢复 Authelia 的有效 email claim 再次登录。

### 必须成立

缺少可用 email 的每次登录均失败，不创建会话或 Owner Scope，也不回退使用历史 email；恢复有效 email 后，相同身份可以登录原 Owner。

### 禁止结果

不得因 `(issuer, subject)` 已绑定而放宽 email 要求，不得创建匿名或缺少 email 映射的会话，不得修改既有 Owner 业务数据。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 已绑定身份缺少 email 时仍拒绝登录 | Missing | 尚无实现测试 |
| 空值和不合法 email 均不能建立 Owner Scope | Missing | 尚无实现测试 |
| 失败不使用历史 email 且不改变既有数据 | Missing | 尚无实现测试 |
| email 恢复后重新解析到原 owner_id | Missing | 尚无实现测试 |

### 决策依据

根决策 D2、不变量 3，以及明确接受的认证可用性代价。

## An unknown subject must never take over an owner by matching email

### 风险

Authelia 重建、subject 配置变化或 email 复用后，未知身份仅凭相同 email 获得既有 Owner 数据。

### 前置状态

Owner A 已绑定 `(issuer-1, subject-1)` 和 email E；准备未知 `(issuer-1, subject-2)` 或 `(issuer-2, subject-1)`，其 email 同为 E。

### 触发

未知身份尝试首次登录并访问 Owner A 的记录。

### 必须成立

系统返回可诊断的身份/email 冲突，不创建或绑定 Owner Scope；Owner A 的 identity、email 和业务记录均保持不变。

### 禁止结果

不得因 email 相等自动绑定、合并 Owner 或迁移业务记录，不得忽略 issuer 只比较 subject。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 未知 subject 与已有 email 冲突时拒绝自动绑定 | Covered | `crates/db/tests/postgres.rs::identity_and_database_constraints_isolate_owners`，真实 PostgreSQL 17，默认 ignore；身份输入为测试提供，不含 OIDC 协议验证 |
| `(issuer, subject)` 两部分共同参与身份匹配 | Covered | `crates/db/tests/postgres.rs::identity_and_database_constraints_isolate_owners`，真实 PostgreSQL 17，默认 ignore；身份输入为测试提供，不含 OIDC 协议验证 |
| 冲突失败不改变 Owner、Identity 或业务记录 | Missing | 尚无实现测试 |

### 决策依据

根决策 D1、D2 及不变量 2。

## Owner scope must come only from the authenticated server context

### 风险

攻击者在路径、query、body、上传文件或同步记录中填写另一 ownerId，从而读取或修改他人数据。

### 前置状态

Owner A、B 各有独立记录；请求已认证为 A，但在各类输入位置提交 B 的 ownerId 和记录 ID。另准备同时包含 A、B 记录的批量请求。

### 触发

执行读取、创建、修改、删除、导入与批量操作，并由后台任务处理 A 的记录。

### 必须成立

所有普通业务行为只使用认证上下文中的 A；自报 B 不授予权限。批量请求遇到 B 的记录时整体失败，后台任务使用持久化 owner_id 重建 A 的范围。

### 禁止结果

不得把 UUID 难猜视为授权，不得静默跳过越权项后部分成功，不得通过 email 为后台任务重新定位 Owner。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 所有外部 ownerId 均不能扩大认证范围 | Missing | 尚无实现测试 |
| 全局记录 ID 查询仍附带 owner_id 条件 | Partial | `crates/db/tests/postgres.rs::identity_and_database_constraints_isolate_owners`，真实 PostgreSQL 17，默认 ignore；身份输入为测试提供，不含 OIDC 协议验证 |
| 混合 Owner 批量请求完整失败 | Missing | 尚无实现测试 |
| 后台任务持久化并恢复原 Owner Scope | Missing | 尚无实现测试 |

### 决策依据

根决策 D4 及不变量 7。

## Owner-scoped relationships must reject cross-owner references

### 风险

子表只校验父 ID 存在，导致 Owner A 的 Message、Import 或任务引用 Owner B 的 Conversation 和 Message。

### 前置状态

Owner A、B 各有 Conversation 和 Message；准备跨 Owner conversation、parent_message、import head 关系，以及同 Owner 的合法关系。

### 触发

绕过常规应用流程直接执行关系写入，并通过正常接口执行相同场景。

### 必须成立

数据库复合外键拒绝全部跨 Owner 组合，同 Owner 合法关系成功；失败不留下部分关系或改变父记录。

### 禁止结果

不得只依赖应用层预查询，不得因子表可以从父表推导而省略 owner_id，不得允许 owner_id 为空。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| Conversation/Message 跨 Owner 引用由数据库拒绝 | Covered | `crates/db/tests/postgres.rs::identity_and_database_constraints_isolate_owners`，真实 PostgreSQL 17，默认 ignore；身份输入为测试提供，不含 OIDC 协议验证 |
| Message 父关系同时保持 owner 与 conversation 一致 | Missing | 尚无实现测试 |
| Import、head 和后台状态不能跨 Owner | Missing | 尚无实现测试 |
| Owner-scoped 表的 owner_id 均不可为空 | Missing | 尚无实现测试 |

### 决策依据

根决策 D3 及不变量 5、6。

## Synchronization must preserve immutable owner assignment

### 风险

客户端利用更大的 updatedAt 改写 owner_id，或复用另一 Owner 的游标，从全局 serverVersion 流中读取、删除或跳过他人数据。

### 前置状态

Owner A、B 在全局 sequence 中拥有交错版本；A 上传一个声称属于 B 且 updatedAt 更大的记录，并尝试使用 B 的 cursor 拉取。

### 触发

执行上传、增量拉取、墓碑处理和 Owner 切换。

### 必须成立

上传记录被强制归属或校验为 A，既有 B 记录不能被修改；拉取只返回 A 的记录，其他版本表现为正常空洞。切换到 B 时使用独立本地范围和 cursor。

### 禁止结果

不得把 owner_id 当作 LWW 业务字段，不得因知道记录 UUID 或 serverVersion 返回 B 的数据，不得跨 Owner 复用游标、墓碑或待同步状态。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 更大 updatedAt 不能改变既有 owner_id | Missing | 尚无实现测试 |
| 全局版本拉取始终按 Owner Scope 过滤 | Missing | 尚无实现测试 |
| 其他 Owner 版本只形成允许的游标空洞 | Missing | 尚无实现测试 |
| Owner 切换隔离 cursor、墓碑和待同步状态 | Missing | 尚无实现测试 |

### 决策依据

根决策 D5 及不变量 8、9。

## Ordinary operations must not transfer or cascade-delete an owner

### 风险

普通更新改变 owner_id，或删除 Owner/Identity 时数据库级联清空全部知识记录，绕过未来迁移和删除协议。

### 前置状态

一个 Owner 具有 Identity、Conversation、Message、Import 和同步状态；准备普通 owner_id 更新、Identity 删除和 Owner 删除操作。

### 触发

通过普通业务接口及数据库外键行为执行这些操作。

### 必须成立

普通更新不能改变 owner_id；删除 Identity 不级联删除 Owner 数据；Owner 删除与跨 Owner 转移在没有后续获批协议时不可执行。

### 禁止结果

不得使用 `ON DELETE CASCADE` 定义 Owner 生命周期，不得形成部分记录已转移、部分仍属于旧 Owner 的状态。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 普通业务更新不能转移记录 Owner | Missing | 尚无实现测试 |
| Identity 删除不会级联删除知识数据 | Missing | 尚无实现测试 |
| 未获批时 Owner 删除和跨 Owner 转移不可执行 | Missing | 尚无实现测试 |

### 决策依据

根决策 D6 及不变量 1、8。

## An inactive session past 24 hours must revalidate on its next request

### 风险

系统把 24 小时复核窗口误实现成空闲过期，导致长期未访问用户无条件丢失 Session；或者反过来直接延长窗口，不向 Authelia 复核已经变化的身份。

### 前置状态

一个未撤销 Palace Session 的 `last_identity_verified_at` 已超过 24 小时，浏览器仍持有有效 cookie。分别准备有效 refresh credential、已过期 credential、Authelia 临时不可达和浏览器 cookie 缺失场景。

### 触发

用户在闲置超过 24 小时后发起下一次受保护请求。

### 必须成立

闲置本身不撤销服务端 Session。refresh 有效时完成复核并继续原请求；refresh 过期时进入新 OIDC 流程，Authelia SSO 有效可无交互返回，否则要求重新认证；Authelia 临时不可达时拒绝本次访问但保留 Session 供重试；cookie 缺失时重新登录。

### 禁止结果

不得仅因超过 24 小时无请求删除 Session，不得未经复核返回业务数据，不得承诺 refresh 过期或 cookie 丢失后仍能靠原 Session 无条件恢复。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 闲置超过 24 小时不自动撤销服务端 Session | Missing | 尚无实现测试 |
| 有效 refresh 在下一次请求完成复核并继续 | Missing | 尚无实现测试 |
| refresh 过期进入 OIDC，按 Authelia SSO 状态决定是否交互 | Missing | 尚无实现测试 |
| Authelia 临时不可达拒绝访问但保留 Session 供重试 | Missing | 尚无实现测试 |
| cookie 缺失不能恢复原 Session | Missing | 尚无实现测试 |

### 决策依据

长期 Session 后续决策 D2、D3 及不变量 2–5。

## A browser session secret must remain opaque and resistant to fixation

### 风险

cookie 暴露 owner_id 或 OIDC token，脚本读取长期凭证，登录后沿用攻击者预设 Session，或状态变更请求只依赖 SameSite 而受到 CSRF。

### 前置状态

准备匿名 Session、正常登录、伪造/篡改 cookie、跨站状态变更请求和多标签页并发轮换场景。

### 触发

建立长期 Session、读取 cookie、执行登录轮换和状态变更请求。

### 必须成立

浏览器只持有高熵 opaque secret；cookie 具有 Secure、HttpOnly、host-only、Path 和 SameSite 约束；登录更换旧 Session，状态变更执行 Origin/CSRF 校验，旧 secret 仅在有界并发窗口内可接受。

### 禁止结果

不得在 cookie 中保存 owner_id、email 或 OIDC token，不得让 JavaScript 读取，不得无限期同时接受轮换前后的 secret。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| cookie 只携带 opaque secret 且安全属性完整 | Missing | 尚无实现测试 |
| 登录后旧匿名/预设 Session 不再有效 | Missing | 尚无实现测试 |
| 状态变更具有独立 Origin/CSRF 防护 | Missing | 尚无实现测试 |
| secret 轮换在并发下有界且最终淘汰旧值 | Missing | 尚无实现测试 |

### 决策依据

长期 Session 后续决策 D1、D2 及不变量 1、3。

## Local session revocation must take effect before external logout succeeds

### 风险

退出或管理员撤销依赖 Authelia 外部调用成功，网络失败后 Palace Session 继续访问；或者状态缓存长期认可已撤销 Session。

### 前置状态

Owner 有多个有效 Session，分别准备退出当前设备、退出全部设备、Identity 禁用、email 缺失、credential 泄露及 Authelia revocation endpoint 失败场景。

### 触发

执行每种撤销事件，并在外部调用成功或失败时立即重放旧 cookie 请求。

### 必须成立

Palace 先持久化对应范围的 revoked_at，随后所有新请求均拒绝；外部 logout/revocation 失败被记录并重试，但不能恢复本地 Session。当前设备和全部设备撤销范围准确。

### 禁止结果

不得等待外部副作用后才本地失效，不得因外部失败回滚撤销，不得由无界缓存继续接受旧 Session。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 当前设备与全部设备撤销范围正确 | Missing | 尚无实现测试 |
| 本地撤销提交后旧 cookie 立即失效 | Missing | 尚无实现测试 |
| Authelia 撤销失败不恢复 Session 且可重试 | Missing | 尚无实现测试 |
| Session 状态缓存不能越过本地撤销 | Missing | 尚无实现测试 |

### 决策依据

长期 Session 后续决策 D4 及不变量 6、7。

## Owner sessions and OIDC credentials must never enter business synchronization

### 风险

owner_session、cookie 或 OIDC credential 被同步到 SQLite、包含在 Owner 数据导出中，或获得业务 serverVersion，从而在其他设备复制认证能力。

### 前置状态

Owner 具有有效和已撤销 Session，并执行业务增量拉取、数据库导出和另一设备恢复。

### 触发

检查同步页、导出内容、serverVersion 分配和另一设备本地数据恢复后的认证状态。

### 必须成立

Session、cookie 和 OIDC credential 均只保留在服务端安全域，不出现在业务同步或 Owner 数据导出中，也不分配业务 serverVersion；另一设备必须独立登录。

### 禁止结果

不得通过复制 SQLite、导出文件或同步响应恢复 Palace Session，不得由客户端提交或 LWW 修改 session.owner_id。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 业务同步不返回 Session 或 OIDC credential | Missing | 尚无实现测试 |
| Owner 数据导出不包含认证凭证 | Missing | 尚无实现测试 |
| Session 不分配业务 serverVersion | Missing | 尚无实现测试 |
| 新设备无法从业务数据复制认证状态 | Missing | 尚无实现测试 |

### 决策依据

长期 Session 后续决策 D5 及不变量 8。
