# 日课与工夫核心测试用例

当前决策：[日课以印章排在日期同一行，只记每天做了几次；工夫是带类型与时长的 Moment](../../../decisions/web/habits/0-habits-as-seals-and-activities-as-moments.md)。数据库约束、提交时检查与 Owner 隔离只能以真实 PostgreSQL 为直接证据；打卡的循环、默认时长与曲线在浏览器里计算，以前端单元测试与组件测试为直接证据。

## A check must be set, never accumulated

风险：打卡请求在网络中断后重试，同一天多记一次；归零留下次数为 0 的行，被当成做过；次数超过每天次数，印章永远填不满或溢出。前置：真实 PostgreSQL，一门每天 2 次的日课。触发：同一天把次数设为 1 两次；设为 2；设为 0；设为 3；把每天次数改成 1 后读取。

必须成立：两次设为 1 后只有一条次数为 1 的打卡；设为 0 后这一天没有打卡；设为 3 被拒绝且原有打卡不变；改小每天次数不改写已有打卡。

禁止结果：重复请求累加次数；存在次数为 0 或超过 10 的打卡。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 设定幂等、0 删除、超过每天次数被拒绝、改每天次数不改历史（真实 PostgreSQL） | Missing | `crates/db/tests/postgres/habits.rs::a_check_must_be_set_never_accumulated` |
| 次数与每天次数的范围校验、错误字段名（领域单元测试） | Missing | `crates/domain/src/habit.rs::times_and_counts_stay_within_one_to_ten` |

决策依据：D2，不变量 1、2、3。

## A retired habit must keep its checks but take no more

风险：停用把历史一并删掉；停用后仍能打卡，或停用的日课继续出现在每天的页面上；历史页把窗口外停用的日课也列出来。前置：真实 PostgreSQL；一门有打卡的日课，一门没有打卡的日课。触发：两门都停用；再停用一次；给停用的日课打卡、改名；读取包含打卡的窗口与不包含打卡的窗口。

必须成立：重复停用成功；停用后打卡与改名都是未找到；包含打卡的窗口返回这门日课（标记为停用）和它的打卡；不包含打卡的窗口与没有打卡的停用日课都不出现。

禁止结果：停用删除了打卡；停用的日课还能写入。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 停用保留打卡、拒绝写入、只在有打卡的窗口里出现（真实 PostgreSQL） | Missing | `crates/db/tests/postgres/habits.rs::a_retired_habit_must_keep_its_checks_but_take_no_more` |

决策依据：D2，不变量 2。

## An activity must be one moment timed at its start

风险：工夫的 Moment 与详情各自漂移，或删除时留下没有详情的 Moment；修改开始时间换了身份，时间线上回不到原来的卡片；时间线不返回工夫的类型与时长。前置：真实 PostgreSQL，一条 07:05 开始、35 分钟的跑步。触发：读取当天的时间线与大纲；把开始时间改到第二天；只删详情；删除这条工夫。

必须成立：时间线在开始那一刻返回工夫卡片，带类型名、分钟数和备注；修改后它出现在第二天且 `id` 不变；只删详情在提交时被拒绝；删除后 Moment 与详情都不在。

禁止结果：没有详情的 `activity` Moment；修改产生新的身份。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 时间线卡片、修改保持身份、单边删除被拒绝、删除成对（真实 PostgreSQL） | Missing | `crates/db/tests/postgres/activities.rs::an_activity_must_be_one_moment_timed_at_its_start` |
| 分钟数与备注的校验（领域单元测试） | Missing | `crates/domain/src/activity.rs::an_activity_lasts_one_minute_to_twelve_hours` |

决策依据：D3，不变量 4、6。

## Activity types must be unique and owned

风险：对话框重复提交生出两个同名类型，曲线把同一种工夫拆成两行；一条工夫引用了别人的类型，读取时泄漏别人的类型名。前置：真实 PostgreSQL；Owner A 与 Owner B 各有一个“跑步”。触发：A 再新建“跑步”；A 用 B 的类型记录工夫；B 读取、修改、删除 A 的工夫。

必须成立：重复新建返回已有的类型且只有一个；引用 B 的类型是未找到且什么也没写；B 对 A 的工夫都是未找到。

禁止结果：同一 Owner 有两个同名类型；工夫的类型属于另一个 Owner。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 同名类型复用、跨 Owner 的类型与工夫都不可见（真实 PostgreSQL） | Missing | `crates/db/tests/postgres/activities.rs::activity_types_must_be_unique_and_owned` |
| 日课与打卡的 Owner 隔离（真实 PostgreSQL） | Missing | `crates/db/tests/postgres/habits.rs::habits_must_stay_in_their_owner_scope` |
| 接口的状态码与响应结构（真实 Router） | Missing | `crates/backend/tests/http/habits.rs::exercise_habits` |

决策依据：D3，不变量 5、7。

## Tapping a seal must cycle the day's count

风险：点印章累加到超过每天次数，或满了以后不能归零；点的是以前的日子却写到了今天；写入失败后印章停在错误的状态。前置：组件测试，假的服务端；一门每天 2 次的日课。触发：在今天点三次；翻到前一天点一次；让一次写入失败。

必须成立：今天的请求依次是 1、2、0，印章依次是部分、做完、没做；前一天的请求写在前一天；失败后印章回到服务端的次数。

禁止结果：请求的次数超过每天次数；写到另一天。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 印章的循环、按显示的那天写入、失败回读（jsdom 组件测试） | Missing | `apps/palace-web/src/pages/habits.test.tsx::tapping a seal cycles the day's count` |
| 下一个次数与填充比例（前端单元测试） | Missing | `apps/palace-web/src/lib/habits.test.ts::next count wraps to zero after the day is full` |

决策依据：D4，不变量 3。

## Recording an activity must read the hours the reader typed

风险：开始时间算到了另一天；`1:30` 被当成 1 分钟；结束晚于现在的工夫被保存；默认时长取了别的类型。前置：前端单元测试与组件测试。触发：在以前的日子填 `07:30` 与 `1:30`；在今天填一段还没结束的工夫；切换类型。

必须成立：开始落在显示的那天 07:30，时长 90 分钟；还没结束的工夫给出原因并禁用保存；切换类型后未手动改过的时长换成那个类型的平均值。

禁止结果：保存一条结束晚于现在或超过 12 小时的工夫。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 时间与时长的解析、校验和默认时长（前端单元测试） | Missing | `apps/palace-web/src/lib/activities.test.ts::reads an activity on the day it is filed under` |
| 对话框记录、修改与删除工夫（jsdom 组件测试） | Missing | `apps/palace-web/src/pages/habits.test.tsx::records an activity from the day menu` |

决策依据：D5。

## The activity curve must never overshoot its days

风险：圆滑曲线在两天之间冲到 0 以下或高过峰值，读成并不存在的时长；没做的那天被连成直线。前置：前端单元测试。触发：对 0、30、0、45、45、0 这样的序列生成曲线。

必须成立：曲线经过每一天的点；每段贝塞尔控制点都落在两端点的纵坐标之间；为 0 的那天落在基线上。

禁止结果：控制点越过相邻两天的值。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 单调插值的控制点范围（前端单元测试） | Missing | `apps/palace-web/src/lib/activities.test.ts::the curve stays between neighbouring days` |

决策依据：D6。
