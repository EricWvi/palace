---
status: implemented
date: 2026-10-18
---

# 用 Owner 级可删除的 Access Token 认证 CLI 与第三方服务，不让它们复用浏览器 Session

Palace 新增 Owner 级 Access Token：用户在「时刻」页日期菜单的“密钥管理”弹窗中填写备注生成 token，CLI 与第三方服务以 `Authorization: Bearer <token>` 调用全部业务 API。token 不过期，删除即失效；Owner 或其 Identity 被禁用时立即不可用。服务端只保存 token 摘要和用于展示的头尾片段，完整值只在创建响应中出现一次；界面任何时候都只显示头尾、中间以星号遮盖，完整值只能在创建后通过“复制”取得一次。token 不能管理 token：生成、列出和删除只接受浏览器 Session。

当前为 `implemented`，核心测试用例见[Access Token](../../../test-cases/server/owner/access-tokens.md)。实现位于 `palace-domain`（token 格式、遮罩与备注校验）、`palace-db` 的 `owner_access_token` 表与认证查询、`palace-backend` 的认证中间件与 `/api/access-tokens` 接口，以及 Web 时刻页菜单与密钥管理弹窗。本决策不改变浏览器 Session 的登录、轮换、24 小时复核和撤销协议；不提供 token 权限分级、有效期或 OAuth 授权流程，也不迁移任何既有凭据。

## 继承与修改

前序决策为[使用可撤销的长期 Palace Session，不设置固定绝对过期时间](20260908-persistent-revocable-owner-sessions.md)，其根决策为[以 Authelia OIDC 身份映射稳定 Owner](0-authelia-identity-and-owner-scoped-records.md)。

| 前序约定 | 本决策 |
| --- | --- |
| Owner Scope 只能来自服务端认证上下文，客户端不能自报 owner | 继承，不变；有效 Access Token 成为第二种认证上下文来源，见 D3 |
| 有效 Palace Session 是唯一的业务请求认证上下文 | 修改；业务路由同时接受 Session 与 Access Token，token 管理路由只接受 Session，见 D3、D4 |
| 每次建立 Owner Scope 前，最近 24 小时内必须经 Authelia 复核身份与 email | 修改；Access Token 不做 OIDC 复核，以“禁用即失效 + 手动删除”约束，Owner Scope 中的 email 取 Owner 当前保存值，见 D2 |
| 凭证是服务端可撤销状态，浏览器只持有高熵 opaque secret，数据库只存摘要 | 继承；token 同样是高熵 opaque secret，数据库只存摘要，见 D1 |
| 有效凭证不因绝对时长或空闲时长自动失效 | 继承；token 不设有效期，见 D2 |
| 不安全方法必须通过 Origin 校验 | 修改；Bearer 请求不做 Origin 校验，Session 请求保持不变，见 D3 |
| 安全凭证不参与业务同步、不分配 `serverVersion`、不随数据导出 | 继承，不变 |
| 新表默认携带 `owner_id NOT NULL`，Owner-scoped 外键包含 owner_id | 继承，不变，见 D1 |

## 问题与约束

Palace 计划提供 CLI，并允许自动化脚本或第三方服务（例如定时导入对话的工具）调用业务 API。这些调用方无法完成浏览器 Authorization Code Flow：没有浏览器跳转、不能安全保管 Authelia refresh token，也不应持有 `__Host-` 前缀的 HttpOnly cookie。若让它们从浏览器复制 Session cookie，会同时继承 cookie 的滚动轮换（旧 secret 30 秒后失效，CLI 很快就会掉线）和“退出当前设备”的撤销范围，且用户无法区分哪个凭证在哪里使用。

调用方通常长期无人值守运行，用户期望配置一次即可持续使用；同时凭证可能写入配置文件、CI secret 或第三方服务的存储，泄露面比浏览器 cookie 更大。因此 token 必须可逐个撤销、可辨认用途，并且泄露后不能扩大自身权限。

## D1：token 是只存摘要的 Owner-scoped opaque secret

新增 `owner_access_token` 表：

| 字段 | 语义 |
| --- | --- |
| `id uuid` | UUID v7，token 的稳定内部身份；用于列表和删除，不作为凭证 |
| `owner_id uuid NOT NULL` | 所属 Owner，创建后不可修改（复用 `reject_owner_change` 触发器） |
| `owner_identity_id uuid NOT NULL` | 创建时所用 Session 的 Identity；`(owner_id, owner_identity_id)` 复合外键到 `owner_identity` |
| `note text NOT NULL` | 用户填写的备注，去除首尾空白后非空，最多 100 个字符 |
| `secret_hash bytea NOT NULL UNIQUE` | 完整 token 的 SHA-256 摘要，唯一定位 token |
| `masked text NOT NULL` | 创建时计算好的遮罩展示值，见下文 |
| `created_at timestamptz NOT NULL` | 毫秒精度，数据库时钟写入 |
| `last_used_at timestamptz` | 最近一次成功认证的时间，毫秒精度；从未使用为 `NULL` |

token 字面形式为 `palace_pat_` 前缀加 32 字节 CSPRNG 随机数的 base64url（无填充，43 个字符）。前缀让 secret scanner 和用户都能一眼认出这是 Palace 凭证，误贴到日志或代码仓库时便于发现与检索。随机部分有 256 bit 熵，因此摘要使用单次 SHA-256 而非慢哈希：慢哈希只对低熵口令有意义，对高熵随机值只增加每次请求的认证开销；这与 Session secret 的处理一致。

`masked` 由前缀、随机部分开头 4 个字符、与隐藏字符数等长的 `*`、随机部分末尾 4 个字符组成，共 35 个 `*`。服务端在创建时一次性算出并保存，此后再也无法从数据库恢复完整值。露出的 8 个字符只用于让用户把列表项与自己手里的配置对上号；剩余 35 个字符仍有约 210 bit 熵，不影响抗猜测能力。

`owner_identity_id` 让 token 与 Session 一样挂在具体 Identity 上：该 Identity 被禁用或解绑时 token 随之失效，未来一个 Owner 绑定多个 Identity 时也能精确撤销。

## D2：token 不过期、不做 OIDC 复核，删除或禁用即失效

token 没有绝对或空闲有效期。删除是唯一的主动撤销方式，删除即物理删除该行；此后同一 token 的请求因找不到摘要而被拒绝，且无法恢复。每次认证都直接读取持久状态并联查 `owner`、`owner_identity` 的 `disabled`，任一为真即拒绝，不依赖额外的撤销触发器，也不缓存有效性。

这里有意放弃前序决策“24 小时内必须经 Authelia 复核”的要求。复核依赖 refresh token 与 UserInfo，而 token 的持有者没有、也不应该有 OIDC 凭证；若要求“Owner 最近 N 天内有浏览器登录复核过才允许 token 使用”，无人值守的 CLI 会在用户一段时间不打开网页后静默失效，违背“配置一次持续使用”的目标。取舍后的安全边界是：

- Authelia 中停用用户不会自动传播到 token；需要在 Palace 侧禁用 Owner/Identity 或删除 token。浏览器 Session 的 24 小时复核在用户下次打开网页时仍会生效。
- token 认证建立的 Owner 使用 `owner.email` 当前保存值（即最近一次 Session 复核或登录写入的值），不代表本次请求经过 email 复核。业务接口只把 email 用于展示，不以它做授权判断，所以不影响 Owner Scope 的正确性。

物理删除意味着不保留已删除 token 的审计记录。当前只有自用场景，列表的意义是“哪些凭证还能用”；若以后需要审计，再由后续决策引入软删除。

`last_used_at` 只在距上次记录超过 60 秒时更新，避免 CLI 连续请求时每次都写一行；它只用于帮助用户判断某个 token 是否仍在使用，不承担安全语义。

## D3：业务 API 接受 `Authorization: Bearer`，与 Session 互斥

业务路由（当前 `business_routes` 与 `media_routes` 中的全部接口）的认证规则：

| 请求携带 | 行为 |
| --- | --- |
| 恰好一个 `Authorization: Bearer palace_pat_…`（scheme 不区分大小写） | 按摘要认证 token；忽略 Session cookie，不刷新、不下发 `Set-Cookie`；不做 Origin 校验 |
| 无 `Authorization` | 维持现有 Session 认证，不安全方法仍要求 Origin 校验 |
| 多个 `Authorization`、非 Bearer scheme、缺少 `palace_pat_` 前缀或字符集不合法 | `401`，不回退到 Session |

Bearer 请求跳过 Origin 校验，是因为 Origin 校验防御的是 CSRF：浏览器会把 cookie 自动附加到跨站请求上，而从不自动附加 `Authorization` 头，攻击页面无法让受害者浏览器带上 token。CLI 与服务端调用方也通常不发送 `Origin`，强制校验只会阻断合法调用。Palace 不开启 CORS，跨站脚本即使持有 token 也读不到响应，这一点不变。

携带 `Authorization` 时不回退 Session，是为了让认证来源唯一、可解释：一个格式错误或已删除的 token 必须明确失败，而不是因为同一浏览器恰好有 cookie 而被静默放行，否则 CLI 用户无法发现自己的 token 已失效。

token 的权限等同于 Session 能调用的全部业务接口，包括导入、修改和删除。只读或按接口分级的权限需要定义 scope 模型和每个接口的归类，超出本次需求；见“本决策未解决的问题”。

所有 Bearer 认证失败统一返回 `401`，不区分“不存在”“已删除”“Owner 已禁用”，避免向持有者泄露账户状态。token 与其摘要不进入日志、错误响应或 OpenAPI 示例。

## D4：token 管理只接受浏览器 Session

| 方法与路径 | 语义 |
| --- | --- |
| `GET /api/access-tokens` | 列出当前 Owner 的 token：`id`、`note`、`masked`、`created_at`、`last_used_at`，按创建时间倒序 |
| `POST /api/access-tokens` | 请求体 `{ "note": "…" }`；返回 `201` 与 `{ "access_token": 列表项, "token": 完整值 }`，完整值只出现这一次 |
| `DELETE /api/access-tokens/{id}` | 在 Owner Scope 内删除；不属于当前 Owner 或不存在都返回 `404` |

这三个接口只接受 Session：请求带任何 `Authorization` 头都返回 `403` 与错误码 `session_required`（与 Origin 被拒的 `origin_rejected` 区分），并遵守 Session 的 Origin 校验。原因是 token 存放位置比浏览器 cookie 更容易泄露：若 token 能生成新 token，攻击者拿到一个就能生成另一个持久后门，用户删除原 token 也无法清除；若 token 能删除 token，泄露的 token 可以删掉用户正在使用的其他凭证。把管理权限留在需要 Authelia 登录的浏览器 Session 上，保证撤销手段始终掌握在完成过身份认证的人手里。

本地单用户开发模式（`run:server`）中，所有请求都归属固定 Owner，token 管理接口与其他业务接口一样直接可用，便于界面开发；该模式本身就不是安全边界，固定 Owner 也不校验 Bearer token。

`created_at`、`last_used_at` 在 API 中按项目惯例使用 epoch 毫秒。

## D5：时刻页菜单的“密钥管理”弹窗

时刻页日期标题旁的 `···` 菜单在“导入对话”下方增加“密钥管理”，打开一个弹窗：

- 顶部是备注输入框与“生成”按钮。备注为空时不能提交；超过长度时显示服务端校验错误。
- 下方是 token 列表，每行第一行是备注与操作，第二行是独占整行宽度的遮罩值，第三行是创建日期和最近使用日期（未使用显示“从未使用”）。遮罩值不截断，窄屏时折行：尾部字符正是用户对号的依据。删除需要二次确认，确认文案说明使用该 token 的程序将立即无法访问。
- 生成成功后，新 token 出现在列表顶部，同样只显示遮罩值，旁边额外提供“复制”按钮与“仅此一次可复制，关闭后无法再次查看”的提示。完整值只保存在弹窗组件的内存状态中，关闭弹窗即丢弃，不写入 React Query 缓存、URL、localStorage 或任何持久位置。

界面从不显示完整 token，是为了避免屏幕共享、截图和旁观泄露；复制按钮让用户仍能把完整值放进 CLI 配置。token 用途与 `Authorization: Bearer` 用法放在“备注”旁的帮助提示里，列表为空时只显示“还没有密钥。”。这偏离了 approved 版本“空列表时常驻一句用法说明”的设想，原因是界面原则要求说明文字按需出现，而用法说明在已有 token 时同样需要，放在提示里两种情况都能看到。

把入口放在时刻页菜单，是沿用[从时刻页日期菜单导入对话](../import/20261004-import-from-day-menu-and-open-reading-page.md) D1 已建立的唯一操作菜单；Palace 目前没有设置页，新建一个只容纳一项功能的页面与“少 UI”的界面原则相悖。日后出现设置页时，入口可以迁移，不影响服务端契约。

## 不变量

1. 数据库只保存 token 的 SHA-256 摘要与遮罩值，任何接口都不能在创建响应之外返回完整 token。
2. 每个 token 属于且仅属于一个 Owner 与其一个 Identity，归属创建后不可修改。
3. token 删除后，使用它的下一次请求必定被拒绝；不存在缓存窗口。
4. Owner 或 token 所属 Identity 被禁用时，该 token 的请求必定被拒绝。
5. 携带 `Authorization` 的请求只按 token 认证，失败时不回退到 Session cookie，也不下发 `Set-Cookie`。
6. token 认证的请求只能调用业务接口，不能调用 token 管理、登录或登出接口。
7. token 认证建立的 Owner Scope 与 Session 认证的 Owner Scope 对业务数据的访问范围完全一致。
8. token 与其摘要不参与业务同步、不随 Owner 数据导出、不写入日志。

## 为什么不是这些替代方案

| 替代方案 | 优势 | 本次取舍 |
| --- | --- | --- |
| 让 CLI 复制浏览器 Session cookie | 不新增认证通道 | cookie 每 24 小时轮换且旧值 30 秒后失效，CLI 很快掉线；退出当前设备会连带撤销；无法区分用途或单独撤销 |
| CLI 走 OAuth 2.0 Device Authorization Grant，由 Authelia 签发 token | 身份复核与撤销全部交给身份提供方，复用 24 小时复核 | 第三方服务需要实现完整 OAuth 流程；refresh token 仍需在调用方长期保存并处理轮换并发，复杂度远超当前自用规模；可作为未来原生客户端的方向 |
| 自包含 JWT（签名 token） | 认证无需查库 | 删除后无法立即失效，违背不变量 3；需要维护撤销列表时又退化为查库 |
| token 有效性依赖 Owner 最近 N 天内有过浏览器复核 | 保留 Authelia 停用的传播 | 无人值守的 CLI 会因用户一段时间没打开网页而静默失效，违背“配置一次持续使用” |
| 慢哈希（Argon2/bcrypt）保存摘要 | 对低熵口令可抗离线爆破 | token 是 256 bit 随机值，离线爆破本就不可行；慢哈希只增加每个请求的认证延迟 |
| 加密保存完整 token，允许事后再次查看 | 用户丢失后可找回 | 数据库或 `PALACE_SESSION_KEY` 泄露即暴露全部 token；丢失时删除重建的成本很低 |
| 软删除保留撤销记录 | 可审计 | 当前自用场景不需要审计，列表只应展示可用凭证；需要时由后续决策引入 |
| 允许 token 管理 token | CLI 可自助轮换 | 泄露的 token 可自我繁殖或删除其他凭证，撤销失去意义 |

## 风险与为什么不能直接改写

token 不过期且不做 OIDC 复核，泄露后在被删除前一直有效，权限覆盖全部业务数据包括删除。缓解手段是：前缀便于 secret scanner 发现、`last_used_at` 帮助识别异常使用、删除立即生效。用户应为每个调用方生成单独的 token，以便泄露时只撤销一个。

token 字面格式（前缀、长度、字符集）和摘要算法一旦发布，就被外部配置长期持有。日后若要更换，不能直接改写解析规则让旧 token 静默失效；应让新旧格式在一段时间内并行认证，或在界面中明确标出需要重新生成的旧 token，由用户主动替换。

## 本决策未解决的问题

- 只读或按资源分级的 token 权限：需要先为每个业务接口定义 scope 归类，超出本次需求。
- 可选有效期：当前选择不过期；若需要，可加可空的 `expires_at` 而不改变既有 token 语义。
- 每个 Owner 的 token 数量上限与认证失败速率限制：自用规模下暂不需要，开放多用户前再决定。
- Authelia 停用用户自动传播到 token：需要 Authelia 侧事件或周期对账机制，另行决策。
- CLI 本身的命令设计与凭证存放位置：属于 `cli` 领域的后续决策。

## 落地与验收

数据层、认证中间件、管理接口、OpenAPI 契约与生成的前端类型、Web 弹窗和文档均已落地。数据库约束、删除与禁用后立即失效、`last_used_at` 节流和 Owner 隔离由真实 PostgreSQL 测试验证；Bearer 与 Session 互斥、管理接口拒绝 token、Origin 规则由真实路由加真实 PostgreSQL 的 HTTP 测试验证，其响应经 OpenAPI 契约校验；界面由组件测试与真实 Chromium e2e 验证。浏览器 Session 的登录、轮换、复核、登出行为不变。服务端流程见[认证系统](../../../../docs/server/认证系统.md)。
