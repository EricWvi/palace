---
status: implemented
date: 2026-09-08
---

# 以 Authelia OIDC 身份映射稳定 Owner，并用 owner_id 隔离业务记录

Palace 使用 Authelia 完成认证，以 OIDC 的 `(issuer, subject)` 将外部身份绑定到内部稳定 `owner.id`，同时维护 Owner 与当前 email 的映射。所有保存 Owner 数据的业务表默认包含不可为空的 `owner_id`；请求中的 Owner Scope 只能由服务端认证上下文确定，客户端不能自报或改写。

当前为 `implemented`，服务端契约已实现并验证；核心测试用例分别记录直接证据及仍属于展示端、设备交互或导出的后续验证边界。本文件是 `server/owner` 的根决策，没有前序 ADR；第一版建立新身份与归属模型，不承担既有生产数据、其他认证提供方或旧客户端的自动迁移义务。Owner 删除、共享知识库和跨 Owner 转移留给后续决策。

## 范围与依赖

拥有 `owner`、`owner_identity` 两张表及认证身份解析、Owner Scope 建立规则。`conversation`、`message`、`conversation_import` 是当前明确的 Owner 数据；[来源会话与消息树](../conversation/0-source-session-and-message-tree.md)、[用户提交线性路径导入](../import/0-user-submitted-linear-path.md)中的“授权范围”第一版具体化为 Owner Scope。[客户端时间戳 LWW 与全局序列同步](../sync/0-client-timestamp-lww-and-global-seq.md)继续使用全局 `serverVersion`，但上传和拉取必须按 Owner Scope 隔离。

Authelia 负责验证用户凭据和执行认证策略；Palace 负责验证 OIDC 结果、映射 Owner、实施业务授权并约束数据归属。Authelia 的用户目录、密码、二次认证设备和会话数据不复制进 Palace 数据库。

## 问题与约束

大部分业务表都直接或间接保存某个用户的知识数据。如果只有顶层 Conversation 带 Owner，而 Message、Import 等子记录只依赖应用代码沿外键回溯，遗漏过滤或错误关联可能越过所有权边界。未来 SQLite 客户端同步也需要从每条记录直接判断其所属范围。

email 适合显示和联系，但可能变化，也不具备 OIDC 稳定身份保证。Authelia 官方建议应用使用 `(iss, sub)` 绑定本地账号，并明确 email、preferred_username 不保证稳定或唯一；Authelia 的 `sub` 用于标识用户，email 作为标准 claim 提供。[Authelia OIDC 账号绑定 FAQ](https://www.authelia.com/integration/openid-connect/frequently-asked-questions/)、[Authelia OIDC Claims](https://www.authelia.com/integration/openid-connect/openid-connect-1.0-claims/)。

## D1：Owner 身份与 Authelia Identity 分离

| 表 | 主要字段 | 关键规则 |
| --- | --- | --- |
| `owner` | `id uuid`, `email`, `created_at`, `updated_at` | `id` 是 Palace 内部稳定身份；email 是当前映射值，不作为业务外键 |
| `owner_identity` | `id uuid`, `owner_id uuid`, `issuer`, `subject`, `created_at`, `last_seen_at` | `(issuer, subject)` 全局唯一；第一版一个 Owner 只绑定一个活跃 Authelia Identity |

Palace 使用自己生成的 `owner.id` 作为所有业务表的归属键，不把 email、Authelia username 或 OIDC subject 直接复制为业务主键。`owner_identity` 负责外部身份绑定，使认证提供方字段与业务身份分离；同一个 Owner 的全部历史数据在 email 改变后仍保持原 `owner_id`。

第一版通过 Authelia OIDC Authorization Code Flow 建立登录会话。Palace 只接受经过 issuer、签名、audience、有效期、state 和 nonce 等协议校验的认证结果，并使用 `(issuer, subject)` 查询 `owner_identity`。具体会话载体由[长期 Session 后续决策](20260908-persistent-revocable-owner-sessions.md)补充；不能信任浏览器直接提交的身份字段。

Authelia client 的 subject 模式或 sector identifier 改变可能使 `sub` 整体变化，官方说明这种配置变化会让 relying party 把后续认证识别为新用户。[Authelia OIDC Client 配置](https://www.authelia.com/configuration/identity-providers/openid-connect/clients/)。部署不能直接修改这类配置后继续沿用旧绑定；必须先制定身份映射迁移并验证每个 Owner 的新旧 subject 对应关系。

## D2：email 是 Owner 当前属性，不是认证锚点

首次见到未知 `(issuer, subject)` 且认证结果提供可用 email 时，可以原子创建 Owner 与 Owner Identity。`owner.email` 保存 Authelia 当前返回的 email，并在已绑定身份后续登录时同步更新；更新 email 不修改 Owner ID，也不迁移任何业务记录。

活跃 Owner 的规范 email 第一版保持唯一，用于避免界面和人工运维把同一地址解释为多个数据所有者。规范化采用去除首尾空白后按大小写不敏感比较，数据库同时保留 Authelia 返回的原始显示值。规范化只服务于 email 冲突检测，不承担认证身份匹配。

未知 `(issuer, subject)` 即使 email 与既有 Owner 相同，也不能自动绑定或接管该 Owner。系统返回身份冲突并停止创建，由经过授权的显式迁移流程核对后处理。否则 Authelia 重建、subject 配置变化或错误复用 email 时可能直接获得另一 Owner 的知识数据。

认证结果缺少 email、email 不合法或与另一 Owner 冲突时，不能建立 Owner Scope。即使 `(issuer, subject)` 已绑定已有 Owner，只要本次 Authelia 认证结果没有可用 email，也必须拒绝登录；不能回退使用 Palace 中上次保存的 email。该规则以登录可用性换取每次会话都具有经过本次认证确认的 Owner/email 映射。

## D3：保存 Owner 数据的表默认携带 owner_id

当前表的归属规则如下：

| 表或数据 | owner_id | 说明 |
| --- | --- | --- |
| `owner` | 否 | 自身就是归属根 |
| `owner_identity` | 是 | 外部身份只能绑定到一个 Owner |
| `conversation` | 是 | 来源会话在 Owner Scope 内唯一 |
| `message` | 是 | 即使可由 conversation 推导，也冗余保存以约束跨 Owner 父子引用 |
| `conversation_import` | 是 | 导入、Conversation 与路径 head 必须属于同一 Owner |
| 同步游标、客户端任务和 Owner 派生索引 | 是 | 状态不能在 Owner 之间复用 |
| PG 全局 sequence 与静态系统配置 | 否 | 不保存某个 Owner 的业务内容 |

尚未建立 Owner 的短期 OIDC 登录尝试仅保存浏览器绑定、state 与协议 proof，不保存或引用业务数据，因此作为全局安全基础设施例外不携带 owner_id。

未来新增表默认加入 `owner_id NOT NULL`。只有同时满足“不包含 Owner 数据、不引用 Owner 数据、不会按 Owner 查询或同步”的全局基础设施表才可省略，并必须在所属 ADR 说明理由；不能以“可从父表推导”为由省略。

所有 Owner-scoped 外键必须把 owner_id 纳入数据库约束。例如 Message 使用 `(owner_id, conversation_id) → conversation(owner_id, id)`；父消息还要保证 owner、conversation 同时一致。`conversation_import` 到 Conversation 和 head Message 同理。父表为复合外键提供对应唯一键，不能只在应用层先查后写。

Owner-scoped 唯一约束和常用索引也必须以 owner_id 开头或包含 owner_id。既有来源会话唯一规则具体化为 `(owner_id, source, session_id)`；同步拉取索引至少支持 `(owner_id, server_version)`，并包含删除标记。

## D4：Owner Scope 只能从认证上下文注入

每次业务请求先由服务端从已验证的 Authelia Identity 解析唯一 `owner_id`，再将其作为 Owner Scope 传入应用和持久化层。路径参数、query、JSON body、上传文件和同步记录中的 `owner_id` 都不具有授权效力；若协议为数据序列化需要携带 ownerId，服务端也必须忽略其归属声明或验证它严格等于认证上下文。

所有读取、修改、删除和唯一性判断都在 Owner Scope 内执行。通过全局业务 ID 读取时仍同时带 owner_id 条件，避免“UUID 难猜”被误当成授权。批量操作中的任一记录不属于当前 Owner 时，整次业务操作失败；不能静默跳过越权项后返回部分成功。

后台任务必须持久化明确 owner_id，并在执行时重新建立同一 Owner Scope；不能使用创建任务的 email 重新定位 Owner。运维或迁移需要跨 Owner 访问时使用独立受审计的管理入口，不复用普通用户接口中的可选 owner 参数。

## D5：owner_id 不参与 LWW，服务端强制同步归属

`owner_id` 是服务端授权字段，不属于客户端可用 `updatedAt` 覆盖的业务字段。上传新记录时服务端写入当前 Owner Scope；更新既有记录时必须先确认其 owner_id 相同，并保持不变。来自客户端的更大 `updatedAt` 不能转移记录归属。

同步拉取继续使用 PG 全局 `serverVersion`，但查询固定为当前 owner_id 下 `serverVersion > cursor`。其他 Owner 的版本形成正常空洞，客户端不得等待。游标、待同步状态、墓碑和派生任务均绑定 Owner Scope；切换 Owner 必须使用独立本地数据范围和游标，不能复用另一 Owner 的 cursor。

全局 sequence 不包含 Owner 数据，因此不增加 owner_id。它只分配增量顺序，不提供读取权限；知道另一个 Owner 的 serverVersion 或记录 UUID 不能访问其数据。

## D6：Owner 转移和删除不得由普通级联完成

第一版不提供业务记录跨 Owner 转移，也不定义 Owner 删除。`owner_id` 创建后不可由普通更新修改；数据库 `ON DELETE CASCADE` 不能作为 Owner 删除产品语义，否则误删认证映射可能级联清空全部知识数据。

将来需要合并 Owner、迁移 Authelia issuer/subject、转移知识或删除账号时，应新增后续 ADR，明确身份核验、引用迁移、同步墓碑、审计、失败恢复与旧客户端重建。执行前必须能枚举受影响记录，事务或分阶段协议不能产生一部分属于旧 Owner、一部分属于新 Owner 的可见状态。

## 不变量

1. Palace Owner 由稳定内部 ID 标识，email、username 和 OIDC subject 都不作为业务外键。
2. Authelia Identity 仅由经过验证的 `(issuer, subject)` 定位，email 不用于自动接管既有 Owner。
3. 首次登录和身份复核要求当前经过验证的 Authelia 结果包含可用 email；复核窗口内恢复 Owner Scope 遵循[长期 Session 后续决策](20260908-persistent-revocable-owner-sessions.md)。
4. Owner 的当前 email 变化不改变 owner_id 或任何业务记录归属。
5. 保存、引用、查询或同步 Owner 数据的表默认包含 `owner_id NOT NULL`。
6. Owner-scoped 父子关系和引用在数据库中拒绝跨 Owner 组合。
7. 普通请求的 Owner Scope 只来自服务端认证上下文，客户端提交的 ownerId 不授予权限。
8. `owner_id` 不参与 LWW 且不能由普通业务更新或同步上传改变。
9. PG 全局 sequence 可以跨 Owner 分配版本，但每次拉取、游标和墓碑都绑定单一 Owner Scope。

## 为什么不是这些替代方案

| 替代方案 | 优势 | 本次取舍 |
| --- | --- | --- |
| 直接用 email 作为 Owner 主键和认证身份 | 表少、便于人工识别 | email 不保证稳定或唯一，修改地址会迫使迁移全部外键，错误复用可能越权 |
| 直接用 Authelia subject 作为业务 owner_id | 少一层映射 | issuer 或 subject 配置迁移会侵入全部业务表，也把外部身份格式变成领域身份 |
| 已绑定 Owner 缺少 email 时沿用旧映射登录 | Authelia claim 暂时异常时可用性更高 | 当前会话无法证明 email 映射仍成立；第一版选择拒绝登录并暴露身份提供方配置问题 |
| 只在 Conversation 等根表保存 owner_id | 数据冗余少 | 子表查询、同步和后台任务容易漏掉父表过滤，数据库难以直接拒绝跨 Owner 引用 |
| 信任反向代理传入的 email/header | 集成简单 | email 不是稳定锚点，直连或 header 清理配置错误会形成身份伪造边界 |
| 允许请求显式选择 ownerId | 便于管理多个范围 | 把身份与授权交给不可信输入；跨 Owner 管理应使用独立受审计入口 |
| 每个 Owner 独立数据库或 schema | 物理隔离强 | 迁移、连接和全局运维成本随 Owner 数增长；第一版选择共享表加数据库约束 |
| 仅依赖 PostgreSQL RLS | 可集中执行读取隔离 | RLS 上下文配置错误仍有风险，且 SQLite 不具备同一机制；可作为后续纵深防御，不能替代显式 owner_id 与复合外键 |

## 风险与为什么不能直接改写

Owner 是全部业务数据的归属根。上线后直接重写 owner_id、删除 Owner 或按 email 自动合并，会同时改变授权、唯一性、同步游标和墓碑含义；必须使用显式迁移、审计和可恢复步骤，不能执行无条件批量 UPDATE 或级联删除。

Authelia issuer、client subject 类型或 sector identifier 改变可能让全部 `(issuer, subject)` 失配。部署变更前必须导出现有绑定、计算或获取新身份并建立已验证映射；不能在首次遇到新 subject 时按相同 email 自动接管旧 Owner。

每张子表冗余 owner_id 增加写入约束和索引空间，也要求所有关系写入同时携带一致 owner_id。这里以存储成本换取直接查询、同步范围和数据库级越权防护；漏加 owner_id 的新表会成为旁路，必须在 schema review 中默认拒绝。

每次登录都要求 email claim，使 Palace 的可用性依赖 Authelia 正确返回该属性。缺失 email 必须产生可观测的认证失败，不能静默使用历史值；修复路径是恢复 Authelia claim 配置后重新登录，不是由 Palace 绕过认证结果。

## 本决策未解决的问题

- Owner 删除、数据导出、保留期限和隐私合规：涉及不可逆数据生命周期，需要独立决策。
- 多个 Authelia Identity 绑定同一 Owner、身份解绑与恢复：第一版只允许一个活跃绑定。
- 多 Owner 共享知识库、成员角色和邀请：当前 Owner Scope 是单一所有者，不推定协作权限。
- 管理员跨 Owner 运维、审计日志和 impersonation：必须有独立认证授权边界，不能复用普通接口。
- PostgreSQL RLS、加密和备份隔离：可作为纵深防御另行评估，不改变本决策的显式 owner_id 规则。

## 落地与验收

Owner/Identity 映射、持久 Session、浏览器绑定登录 state、加密凭据、24 小时复核、并发轮换、Origin 校验与本地先撤销均已落地。身份协议由签名 token 单元测试和真实 Authelia 4.39.20 契约测试验证；数据库事务、禁用、撤销和隔离由真实 PostgreSQL 验证。共享、Owner 删除、身份迁移、设备 UI 与导出仍按上述后续边界处理。

运行 `task test` 验证默认单元测试与 lint；`task test:integration`、`task test:contract` 显式运行默认忽略的容器测试。具体职责、接口、容量和部署配置见[运行文档](../../../../docs/README.md)，验证证据见对应领域核心测试用例。
