# 导入入口与成功落点核心测试用例

当前决策：[从时刻页日期菜单导入对话](../../../decisions/server/import/20261004-import-from-day-menu-and-open-reading-page.md)，更新入口由[分支管理改为分支列表](../../../decisions/server/conversation/20261006-branch-list-and-update-from-title-menu.md) D1 放宽。导入的校验、幂等与事务语义见[明确目标的线性导入](linear-path-import.md)；菜单的显示与键盘规则见[文字导航与时刻地址](../../web/navigation/text-nav-and-day-routes.md#hover-menus-must-stay-reachable-by-keyboard-and-touch)。

## New imports must start from the day menu with the viewed day as default

风险：在过去某天补录时默认时间仍是今天，导入的对话落到错误日期；或导入入口出现在多处。前置：时间线分别浏览今天和过去某天。触发：从日期标题旁的菜单选择“导入对话”，不改时间直接提交；再把日期和时间改成其他值后提交。

必须成立：浏览今天时默认发生时间为当前本地时间，浏览其他日期时为该日期加上当前本地时刻；提交的 `occurred_at` 等于表单中用户确认的值；点击 `···` 不会打开日期选择器。新建导入只能从日期菜单发起，新建分支只能从阅读页的管理分支发起，追加更新只能从阅读页的标题菜单“更新分支”或管理分支发起。禁止服务端或前端在提交后改写用户确认的时间。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 两种浏览日期下的默认发生时间，提交值等于表单值 | Covered | `app.test.tsx::imports from the day menu at the viewed day and opens the reading page dated to it`、`lib/day.test.ts::bounds a day by local midnights and keeps the time of day for new records`、`e2e/timeline.spec.ts`（改为其他日期后提交） |
| 日期菜单与日期选择器互不触发，导入入口唯一 | Partial | `app.test.tsx::opens the day menu from the keyboard and returns focus on Escape`（菜单只有导入一项）；与日期选择器互不触发没有直接测试 |

决策依据：D1、D2，不变量 1、2。

## Successful import must open the reading page dated to the submitted occurrence

风险：导入后回到导入前浏览的日期，看不到刚导入的对话而误以为失败；或后退回到已关闭的导入对话框状态。前置：浏览 9 月 30 日，导入时把发生日期改为 10 月 2 日；另准备一次幂等重试与一次失败响应。触发：提交新建导入、分支导入和追加更新。

必须成立：成功后先把当前时间线条目替换为 `/?date=<本次 occurred_at 的本地日期>&moment=<本次 Path>`，再打开 `/conversations/<id>?path=<本次 Path>&date=<同一日期>`，返回链接显示 10 月 2 日并能在那天的时间线上定位新 Moment；幂等重试命中已有结果时按返回的 Path 跳转；分支与追加成功后阅读页切到本次 Path，返回日期改为本次提交的日期。失败时不跳转，表单保留输入并可重试。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 发生日期与浏览日期不同时，返回日期等于发生日期 | Covered | `e2e/timeline.spec.ts`（真实 Chromium） |
| 时间线条目替换为发生日期，后退与返回链接到达同一天；幂等重试按返回的 Path 跳转 | Covered | `app.test.tsx::imports from the day menu at the viewed day and opens the reading page dated to it`、`app.test.tsx::reuses an idempotency key on retry after an ambiguous failure`、`e2e/timeline.spec.ts` |
| 分支与追加成功后切到本次 Path 并更新返回日期 | Covered | `pages/conversation.test.tsx::switches to an imported branch and dates the way back to its occurrence`、`e2e/branches.spec.ts` |
| 失败不跳转，保留输入 | Covered | `app.test.tsx::rejects malformed JSON before sending and keeps the form for correction`、`app.test.tsx::reuses an idempotency key on retry after an ambiguous failure` |

决策依据：D3，不变量 3、4、5。
