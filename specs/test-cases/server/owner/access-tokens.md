# Access Token 核心测试用例

本文跟踪[Owner 级 Access Token 后续决策](../../../decisions/server/owner/20261018-owner-access-tokens-for-cli-and-services.md)中 token 保管、撤销、认证互斥、管理权限和 Owner 隔离的长期风险。数据库约束、删除与禁用后的失效只能以真实 PostgreSQL 测试为直接证据；HTTP 认证路由的行为以真实路由加真实 PostgreSQL 的测试为直接证据，fake 或内存实现最多标为 `Partial`；界面展示以组件测试或真实 Chromium e2e 为证据。

## The full access token must leave the server only in its creation response

### 风险

数据库、列表接口或日志中保存或返回完整 token，数据库备份、屏幕共享或日志泄露即可直接冒用凭证。

### 前置状态

一个已登录的 Owner。

### 触发

通过 `POST /api/access-tokens` 生成 token，随后调用 `GET /api/access-tokens`，并直接读取 `owner_access_token` 行。

### 必须成立

创建响应包含一次完整 token，形如 `palace_pat_` 加 43 个 base64url 字符；数据库只保存其 SHA-256 摘要与遮罩值；列表只返回遮罩值，遮罩值保留前缀、随机部分头尾各 4 个字符，其余字符全部为 `*`。

### 禁止结果

不得在数据库任何列、列表响应、错误响应或日志中出现完整 token 或其随机部分超过头尾 8 个字符的片段。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| token 字面格式与遮罩规则正确 | Missing | — |
| 数据库行只含摘要与遮罩值，摘要可定位 token | Missing | — |
| 列表响应不含完整 token | Missing | — |

### 决策依据

D1、D4，不变量 1、8。

## An access token must stop working as soon as it is deleted or its owner is disabled

### 风险

删除 token 或禁用 Owner/Identity 后，已泄露的 token 仍能在缓存窗口或遗漏的状态检查中访问业务数据。

### 前置状态

一个 Owner 持有两个有效 token，两者都已成功调用过业务接口。

### 触发

删除其中一个 token 后立刻用它请求；分别禁用 token 所属 Identity 与 Owner 后用另一个 token 请求。

### 必须成立

被删除的 token 下一次请求即返回 `401`，另一个 token 不受影响；Identity 或 Owner 被禁用后，其全部 token 的请求均返回 `401`；所有失败的响应体相同，不区分原因。

### 禁止结果

不得存在任何删除后仍可认证的窗口，不得因 Authorization 失败而回退到 Session，不得在失败响应中暴露账户状态。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 删除后下一次认证必定失败，其他 token 不受影响 | Missing | — |
| Identity 禁用后其 token 失效 | Missing | — |
| Owner 禁用后其 token 失效 | Missing | — |
| 失败原因不可区分 | Missing | — |

### 决策依据

D2、D3，不变量 3、4。

## A request carrying Authorization must authenticate only by its access token

### 风险

同一请求同时带 cookie 与 Authorization 时选错凭证：无效 token 因 cookie 有效而被放行，使 CLI 用户无法发现 token 已失效；或 token 请求触发 Session 轮换并下发 `Set-Cookie`。

### 前置状态

一个 Owner 持有有效浏览器 Session 与一个有效 token；另准备一个已删除 token 和若干格式错误的 Authorization 头。

### 触发

分别以有效 token、已删除 token、多个 Authorization 头、非 Bearer scheme、缺少前缀的值请求业务接口，部分请求同时携带有效 Session cookie；以有效 token 发送不带 Origin 的不安全方法请求。

### 必须成立

有效 token 建立与 Session 相同的 Owner Scope，响应不含 `Set-Cookie`，不安全方法不要求 Origin；其余情况一律 `401`，即使同时携带有效 Session cookie；不带 Authorization 的请求仍按现有 Session 规则认证并校验 Origin。

### 禁止结果

不得因存在有效 cookie 而放行无效 token，不得为 token 请求刷新或下发 Session cookie，不得放松 Session 请求的 Origin 校验。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 有效 token 访问业务接口且不下发 cookie | Missing | — |
| 无效或格式错误的 Authorization 不回退 Session | Missing | — |
| Bearer 不安全方法不要求 Origin，Session 仍要求 | Missing | — |

### 决策依据

D3，不变量 5、7。

## An access token must never manage access tokens

### 风险

泄露的 token 生成新的持久后门，或删除用户正在使用的其他 token，使撤销失去意义。

### 前置状态

一个 Owner 持有有效浏览器 Session 与一个有效 token。

### 触发

以 token 调用 `GET`、`POST /api/access-tokens` 与 `DELETE /api/access-tokens/{id}`；以 Session 调用同样三个接口，其中不安全方法分别带与不带正确 Origin。

### 必须成立

token 调用三个管理接口均返回 `403` 且无副作用；Session 调用成功，不安全方法缺少正确 Origin 时返回 `403`。

### 禁止结果

不得以 token 创建、列出或删除任何 token。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| token 调用管理接口被拒且不产生新行或删除行 | Missing | — |
| Session 管理接口遵守 Origin 校验 | Missing | — |

### 决策依据

D4，不变量 6。

## Access tokens must stay within their owner

### 风险

一个 Owner 可以列出或删除另一 Owner 的 token，或 token 认证后访问其他 Owner 的业务数据，或 token 归属被改写。

### 前置状态

两个 Owner 各持有 token 与业务数据。

### 触发

Owner A 列出 token、删除 Owner B 的 token ID，以 A 的 token 读取 B 的业务记录；直接 UPDATE `owner_access_token.owner_id`。

### 必须成立

列表只含本 Owner 的 token；跨 Owner 删除与不存在的 ID 都返回 `404` 且 B 的 token 仍有效；A 的 token 只能访问 A 的数据；数据库拒绝修改 token 归属，复合外键拒绝不属于该 Owner 的 Identity。

### 禁止结果

不得跨 Owner 读取、删除 token 或业务数据，不得改写 token 归属。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 列表与删除限定在 Owner Scope 内 | Missing | — |
| token 认证的业务访问限定在 Owner Scope 内 | Missing | — |
| `owner_id` 不可修改，Identity 必须属于同一 Owner | Missing | — |

### 决策依据

D1、D4，不变量 2、7；根决策 D3、D4。

## The key manager must never show a full token

### 风险

密钥管理弹窗在屏幕上显示完整 token，或把完整值写入持久位置，在屏幕共享、截图或浏览器存储中泄露。

### 前置状态

已登录的 Owner 打开时刻页。

### 触发

从日期菜单“导入对话”下方的“密钥管理”打开弹窗，填写备注生成 token，复制，关闭后重新打开；删除一个 token。

### 必须成立

新 token 与列表中的 token 都只显示遮罩值；“复制”写入剪贴板的是完整 token；关闭弹窗后重新打开，复制按钮不再出现；删除前要求二次确认，确认后该项从列表消失。

### 禁止结果

不得在页面文本、URL、React Query 缓存或浏览器存储中出现完整 token。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 页面只显示遮罩值，复制得到完整值 | Missing | — |
| 关闭后无法再次复制 | Missing | — |
| 删除需要确认并刷新列表 | Missing | — |

### 决策依据

D5，不变量 1。
