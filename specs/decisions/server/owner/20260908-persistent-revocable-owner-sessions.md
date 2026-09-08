---
status: approved
date: 2026-09-08
---

# 使用可撤销的长期 Palace Session，不设置固定绝对过期时间

Palace 在 Authelia OIDC 登录成功后建立服务端持久化的 opaque Session。Session 不设置产品级固定绝对寿命，也不因空闲自动过期；浏览器 cookie 在使用中滚动续期，使内网用户可以长期保持登录。同时，Session 必须可单独或按 Owner 撤销，并最迟每 24 小时通过 Authelia 重新确认 identity 与 email；不能用永不过期 JWT 或永不复核的 cookie 绕过认证变化。

当前为 `approved`，决策已经评审通过但尚未实现；对应 Owner 核心测试用例用于跟踪实现证据。影响 Web 登录、OIDC token 保管、服务端 session 持久化、Owner Scope 建立与退出登录。本文件是 `server/owner` 根决策后的第一份后续 ADR；第一版建立新会话协议，不兼容其他 cookie/JWT 会话，也不迁移既有登录状态。

## 继承与修改

前序决策为[以 Authelia OIDC 身份映射稳定 Owner，并用 owner_id 隔离业务记录](0-authelia-identity-and-owner-scoped-records.md)。

| 前序约定 | 本决策 |
| --- | --- |
| 以经过验证的 `(issuer, subject)` 定位 Owner | 继承，不变；Session 只缓存已经建立的绑定，不创造第二种身份匹配规则 |
| 每次建立 Owner Scope 都要求本次 Authelia 结果包含可用 email | 修改；首次登录和每次身份复核必须取得可用 email，复核后 24 小时内可由有效 Palace Session 恢复 Owner Scope |
| Owner Scope 只能来自服务端认证上下文 | 继承，不变；有效 Palace Session 成为后续请求的认证上下文来源 |
| 具体库和会话载体属于实现细节 | 修改；采用服务端持久化 opaque Session 和浏览器持久 cookie |
| Owner/Identity 删除与恢复尚未决定 | 继承边界；本决策只规定禁用、解绑或显式撤销时已有 Session 如何失效 |

## 问题与约束

Palace 主要运行在内网，用户希望完成一次 Authelia 登录后长期稳定使用，不因短时间空闲、服务重启或 OIDC access token 到期频繁重新输入凭据。但内网不是凭证安全边界：浏览器 cookie 仍可能经共享设备、恶意扩展、日志、备份或 XSS 泄露；Authelia 用户也可能被禁用、email 改变或身份绑定被撤销。

Authelia 自身 Session 配置区分 inactivity、expiration 与 remember-me，OIDC access token、ID token、refresh token 也具有各自 lifespan。[Authelia Session 配置](https://www.authelia.com/configuration/session/introduction/)、[Authelia OIDC Provider 配置](https://www.authelia.com/configuration/identity-providers/openid-connect/provider/)。Authelia OIDC 提供 token、UserInfo、introspection 与 revocation endpoint，Palace 可以用短期 token 配合 refresh/revocation，而不把某个 bearer token 变成永久身份。[Authelia OIDC Integration](https://www.authelia.com/integration/openid-connect/introduction/)。

## D1：Palace Session 是服务端状态，不是永久 OIDC token

新增 `owner_session` 表：

| 字段 | 语义 |
| --- | --- |
| `id uuid` | Session 稳定内部身份，不直接作为浏览器凭证 |
| `owner_id uuid` | Session 所属 Owner，创建后不可修改 |
| `owner_identity_id uuid` | 首次登录时验证通过的 Authelia Identity |
| `secret_hash` | 浏览器 opaque secret 的单向摘要，原 secret 不落库 |
| `created_at` | 首次建立时间，仅用于审计，不触发绝对过期 |
| `last_seen_at` | 最近成功使用时间，仅用于观测和清理策略，不触发空闲过期 |
| `last_identity_verified_at` | 最近一次成功从 Authelia 确认 identity 与 email 的时间 |
| `refresh_credential` | 服务端加密保存的 OIDC refresh credential 或等价可撤销材料 |
| `rotated_at` | 最近一次 opaque secret 或 refresh credential 轮换时间 |
| `revoked_at`, `revoke_reason?` | 撤销状态与原因；撤销后不可恢复为同一 Session |

浏览器只保存高熵随机 opaque secret，不保存 owner_id、email、OIDC access token、ID token 或 refresh token。Palace 根据摘要定位有效 Session，再从服务端记录建立 Owner Scope。数据库泄露不能仅凭 `secret_hash` 直接重放浏览器会话，浏览器 cookie 泄露也不能读取服务端保存的 OIDC refresh credential。

OIDC access/ID token 保持短期，不因 Palace Session 长期存在而延长为永久 token。需要延续认证时使用 refresh grant 获取新 token，并用最新 OIDC/UserInfo 数据执行身份复核；Authelia 支持 refresh token 获取新的 token，且建议短期 access/ID token 配合撤销能力。[Authelia OIDC Provider 配置](https://www.authelia.com/configuration/identity-providers/openid-connect/provider/)、[Authelia OIDC FAQ](https://www.authelia.com/integration/openid-connect/frequently-asked-questions/)。

## D2：服务端 Session 不设绝对或空闲过期，cookie 滚动续期

有效 `owner_session` 不设置 `expires_at`，也不因 `created_at` 或 `last_seen_at` 达到某个时长自动失效。服务重启和滚动部署后仍从共享持久化存储恢复 Session，不依赖进程内内存。

浏览器 cookie 使用 `Secure`、`HttpOnly`、host-only、`Path=/` 与适合 OIDC 跳转的 `SameSite=Lax`；不允许 JavaScript 读取。cookie 使用浏览器可接受的有限持久期限，并在有效请求或成功身份复核后滚动续期。浏览器删除 cookie、清理站点数据或限制持久 cookie 时仍需重新登录；“不过期”是 Palace 不主动设置绝对/空闲失效规则，不是对浏览器永久保存的承诺。

状态变更请求仍需要 Origin/CSRF 校验；`SameSite` 和内网部署不能替代 CSRF 防护。登录成功时必须更换已有匿名或旧 Session，防止 session fixation。opaque secret 需要定期或在身份复核后轮换；并发标签页的短暂旧 secret 如何容忍由实现确定，但旧 secret 不能无限期并行有效。

## D3：最迟每 24 小时通过 Authelia 复核 identity 与 email

每个请求先检查本地 Session 是否有效。距离 `last_identity_verified_at` 未满 24 小时时，可以直接从 Session 建立 Owner Scope；达到 24 小时后，必须在返回受保护业务数据前完成一次 Authelia 身份复核。

超过 24 小时没有任何请求本身不撤销 `owner_session`。下一次携带有效 Palace cookie 的请求按以下顺序恢复：

| 状态 | 行为 |
| --- | --- |
| refresh credential 仍有效 | 刷新 OIDC token、完成身份复核并继续原请求；Session 保持同一内部身份 |
| refresh credential 已过期或被撤销 | 原 Session 不能直接续期，进入新的 OIDC Authorization Code Flow；Authelia SSO/remember-me 仍有效时可以无交互返回，否则要求用户重新认证 |
| Authelia 临时不可达 | 本次请求拒绝访问但保留本地 Session，后续请求重新尝试复核 |
| Palace cookie 已被浏览器删除或过期 | 无法定位原 Session，进入新的 OIDC 登录流程 |

因此“长期不过期”保证 Palace 不因闲置时长主动销毁服务端 Session，不保证任意长时间离线后一定能靠旧 refresh credential 无交互恢复；后者还受浏览器 cookie 和 Authelia credential/session 生命周期约束。

身份复核至少确认：

1. refresh/token 交换与 OIDC 响应通过 issuer、签名、audience、有效期等协议校验；
2. `(issuer, subject)` 与 `owner_identity_id` 当前绑定完全一致；
3. 本次结果包含可用 email，且按根决策更新或校验 Owner email 映射；
4. Owner、Owner Identity 与 Owner Session 均未被本地禁用或撤销。

复核成功后原子更新 email 映射、refresh credential、`last_identity_verified_at` 和需要轮换的 secret，再继续请求。并发请求只允许一项复核成功发布新 credential；其他请求复用已提交结果或安全重试，不能用旧 refresh token 并发刷新后互相撤销。

Authelia 不可达、refresh 被拒绝、协议校验失败、identity 不匹配、email 缺失/冲突时，当前请求不能建立 Owner Scope。确定性身份失败撤销 Session；临时网络失败可以保留数据库 Session 供稍后重试，但在成功复核前持续拒绝访问，不能回退到历史 email 或把 24 小时窗口无限延长。

24 小时是外部身份变化传播到 Palace 的最长主动复核窗口，不表示 Authelia 一定在此时间内使所有凭据失效。若部署能接收可靠的注销或撤销通知，可以提前失效；不能因此取消周期复核。

## D4：长期 Session 必须随关键事件可撤销

以下事件必须使相关 Session 失效：

| 事件 | 撤销范围 |
| --- | --- |
| 用户退出当前设备 | 当前 owner_session |
| 用户选择退出全部设备 | 该 Owner 的全部 owner_session |
| Owner 或 Owner Identity 被禁用、解绑 | 关联的全部 owner_session |
| 身份复核发现 subject 改变、email 缺失/冲突或 refresh 被明确拒绝 | 当前 owner_session；身份级异常可撤销该 Identity 的全部 Session |
| 管理员确认 cookie/credential 泄露 | 指定 Session、Identity 或 Owner 的全部 Session |
| Session secret 重放、轮换异常或其他完整性失败 | 当前 owner_session，并记录原因 |

撤销先在 Palace 数据库持久化 `revoked_at`，之后当前及后续请求都被拒绝；清除 cookie 和调用 Authelia revocation/logout 是必须尝试的外部副作用，但外部调用失败不能回滚本地撤销。失败的 Authelia token revocation 需记录并重试，不能让 Palace Session 恢复有效。

每次鉴权都读取或命中有明确失效边界的 Session 状态缓存。缓存不能无限期认可已撤销记录；具体缓存 TTL 由实现性能测试确定，但退出当前设备和管理员撤销在 Palace 接收成功后必须对新请求立即生效。

## D5：Session 归属 Owner，但不参与业务同步

`owner_session`、OIDC token 和 cookie 都是服务端安全状态，不进入 Web/SQLite 业务记录同步，不分配业务 `serverVersion`，也不随 Owner 数据导出。客户端需要独立登录后取得自己的 Palace Session；不能从另一台设备复制数据库或 cookie 恢复认证。

`owner_session.owner_id` 遵守 Owner 根决策，不能由客户端提交或 LWW 修改。后台清理可以物理删除已撤销且超过审计保留期的 Session，但不能删除仍有效 Session 来模拟普通过期。审计保留期限另行配置，不改变撤销立即生效的语义。

## 不变量

1. Palace Session 是可撤销的服务端状态，浏览器 opaque secret 不携带 Owner 身份或 OIDC token。
2. 有效 Palace Session 不因绝对时长或空闲时长自动失效。
3. 浏览器 cookie 使用有限持久期限滚动续期；Palace 不承诺浏览器永久保留 cookie。
4. 闲置超过 24 小时不主动撤销 Palace Session；下一次请求必须先成功复核或重新完成 OIDC 登录才能建立 Owner Scope。
5. 每次身份复核都必须确认同一 `(issuer, subject)` 并取得可用 email，不能使用历史 email 降级。
6. 确定性身份失败、退出登录、禁用、解绑或显式撤销会使相关 Palace Session 不可恢复地失效。
7. Palace 本地撤销先于 Authelia 外部撤销副作用生效，外部失败不能恢复本地 Session。
8. Owner Session、OIDC credential 与 cookie 不参与业务记录同步或 Owner 数据导出。

## 为什么不是这些替代方案

| 替代方案 | 优势 | 本次取舍 |
| --- | --- | --- |
| 永不过期的自包含 JWT | 请求无需查服务端状态 | Owner 禁用、退出和凭证泄露后无法及时撤销，email 规则变化也无法反映 |
| 永不复核 Authelia 的数据库 Session | Authelia 短时不可用不影响使用 | 已禁用用户和失效身份可永久访问，Palace 变成第二个独立身份源 |
| 每个请求都实时访问 Authelia | 身份变化传播最快 | 可用性和延迟完全耦合认证服务，不满足稳定使用目标 |
| 固定 30/90 天绝对过期 | 风险窗口明确 | 正常活跃用户仍被周期性强制重新登录；选择周期复核和可撤销性控制风险 |
| 仅依赖 Authelia remember-me cookie | 少维护一套 Session | Palace OIDC 回调、Owner Scope、退出和多设备撤销仍需要本地状态，且无法独立审计设备会话 |
| 将 refresh token 放入浏览器 | 服务端少保存敏感状态 | 浏览器泄露可直接长期调用 OIDC，难以约束用途和轮换并发 |
| Authelia 临时不可达时无限宽限 | 内网故障时持续可用 | 身份有效性窗口失去上界，等同取消周期复核 |

## 风险与为什么不能直接改写

没有绝对和空闲过期意味着失窃 cookie 在未被发现、未到身份复核或未触发撤销前持续有效；24 小时复核不能替代设备保护、TLS、XSS/CSRF 防护和主动 Session 管理。内网只降低部分暴露面，不改变 bearer secret 被持有者即可使用的事实。

周期复核使 Authelia 故障最长在 24 小时后阻止已有 Session 继续访问。这里接受认证可用性依赖，以维持“每次复核必须有当前 email”和禁用可传播的边界；不得临时修改数据库时间或跳过校验延长窗口。

opaque secret、refresh credential 加密方式或轮换协议投入使用后不能直接替换而让所有旧 secret 同时长期有效。升级应支持有界双读/轮换并记录协议版本，验证新凭证已发布后撤销旧凭证；若无法安全迁移，明确使旧 Session 失效并要求重新登录。

## 本决策未解决的问题

- 24 小时复核上限是否需要按部署缩短：第一版固定最大值，只有后续决策可以放宽，配置可以缩短。
- 多设备 Session 管理 UI、设备命名和最近活动展示：不影响服务端可撤销性，可在交互需求明确后增加。
- Owner 删除、Identity 解绑流程的发起权限与审计保留期：本决策只规定这些事件一旦成立必须撤销 Session。
- Authelia 单点故障的高可用部署：属于基础设施边界，不能通过无限宽限绕过。
- 原生客户端的 cookie 容器、安全存储与 OIDC PKCE：未来客户端接入前另行决定，不复用浏览器 secret 导出。

## 落地顺序

1. 实现 owner_session、opaque cookie、CSRF 防护和登录轮换，以重启恢复、多标签页、cookie 伪造、session fixation 和浏览器清理场景验收。
2. 实现 refresh/UserInfo 复核和撤销流程，以闲置超过 24 小时、email 变化/缺失、Authelia 不可达、refresh 过期/并发、退出当前/全部设备和管理员撤销场景验收。
3. 补齐核心测试用例的直接证据；实现与批准决策一致后同步文档并转为 `implemented`。Owner 删除、Identity 解绑和客户端 Session 复制仍保持未开放。
