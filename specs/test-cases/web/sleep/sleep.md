# 睡眠核心测试用例

当前决策：[睡眠从时刻页菜单开始，计时与结束后的这一晚都以表盘置顶当天](../../../decisions/web/sleep/0-sleep-started-from-the-day-menu-pinned-as-a-dial.md)。数据库约束、提交时检查与事务回滚只能以真实 PostgreSQL 为直接证据；钟点规则在浏览器里判断，以前端单元测试与组件测试为直接证据。

## A night must be one moment whose wake time is its place on the timeline

风险：一晚的 Moment 与详情各自漂移，时间线把它放在一天、表盘和历史页放在另一天；手工写库或迁移只改了一处；超过一天的“夜晚”被存下。前置：真实 PostgreSQL，一晚 23:30 入睡、次日 06:30 醒来。触发：读取醒来前后的时间线；单独改 `moment.occurred_at`、单独改 `sleep.span`、只删详情、把时间段改成 25 小时。

必须成立：时间线在醒来那一刻返回这一晚，卡片带入睡时间；四种单边写入都在提交时被拒绝，这一晚保持原样。

禁止结果：留下没有详情的睡眠 Moment，或 `upper(span)` 与 `occurred_at` 不相等的一晚。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 时间线按醒来时间返回睡眠卡片，单边写入与超长时间段被数据库拒绝（真实 PostgreSQL） | Covered | `crates/db/tests/postgres/sleep.rs::a_night_must_be_one_moment_ending_at_its_wake_time` |
| 时间段的长度与范围校验、错误字段名（领域单元测试） | Covered | `crates/domain/src/sleep.rs::a_night_must_have_length_and_last_at_most_a_day` |

决策依据：D3，不变量 1、2。

## Nights must never overlap

风险：补记、修改与结束各自写入，同一段时间出现两晚，平均时长和 7 晚柱都被算重；另一个 Owner 的夜晚挡住了自己的记录。前置：真实 PostgreSQL；Owner A 有一晚，Owner B 有同一时段的一晚。触发：A 补记一段与已有一晚重叠的夜晚；A 补记一段与已有一晚首尾相接的夜晚；A 把相接的那一晚改成重叠。

必须成立：重叠的补记与修改返回冲突，数据不变；相接的一晚可以记下；B 的夜晚不受影响，也不妨碍 A。

禁止结果：同一 Owner 的两晚时间段相交；重叠写入留下一半。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 排他约束拒绝重叠、允许相接、按 Owner 区分（真实 PostgreSQL） | Covered | `crates/db/tests/postgres/sleep.rs::nights_must_never_overlap` |
| 重叠在接口上是 409（真实 Router） | Covered | `crates/backend/tests/http.rs::authenticated_http_imports_preserve_scope_and_file_parity`（经 `tests/http/sleep.rs::exercise_sleep`） |

决策依据：D3，不变量 3。

## Starting must replace any earlier start

风险：忘记结束的开始一直留着，下一次开始被拒绝或出现两个计时；重复点击开始积累多行。前置：真实 PostgreSQL；一个 20 小时前的开始。触发：再次开始。

必须成立：只剩一个开始，时间是新的；开始不写入任何一晚。

禁止结果：同一 Owner 有两个开始；开始被当成一晚出现在时间线上。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 每个 Owner 至多一个开始，再次开始替换它（真实 PostgreSQL） | Covered | `crates/db/tests/postgres/sleep.rs::starting_must_replace_any_earlier_start` |

决策依据：D4，不变量 4。

## Ending must turn the seen start into one night exactly once

风险：另一台设备已经重新开始，这台设备的停止按钮结束了别人眼中的那一晚；结束请求在网络中断后重试，写出第二晚；结束失败（超过一天、与补记的一晚重叠）却已经删除了开始，计时凭空消失。前置：真实 PostgreSQL；一个 7 小时前的开始；一个 25 小时前的开始；一个 2 小时前的开始和覆盖其中一段的补记。触发：用不同的 `started_at` 结束；结束两次；结束超长的开始；结束被补记挡住的开始。

必须成立：不一致的 `started_at` 是未找到，开始保留；第二次结束返回同一晚，开始已不在；超长与重叠的结束分别是输入错误与冲突，开始保留，补记不变。

禁止结果：一个开始变成两晚；失败的结束删掉开始或写下一半。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 只结束看到的开始，重复结束幂等（真实 PostgreSQL） | Covered | `crates/db/tests/postgres/sleep.rs::ending_must_turn_the_seen_start_into_one_night_once` |
| 失败的结束整体回滚并保留开始（真实 PostgreSQL） | Covered | `crates/db/tests/postgres/sleep.rs::a_failed_end_must_keep_the_start` |
| 不一致的开始在接口上是 404（真实 Router） | Covered | `crates/backend/tests/http.rs::authenticated_http_imports_preserve_scope_and_file_parity`（经 `exercise_sleep`） |

决策依据：D4，不变量 5、6。

## Correcting or deleting a night must move or remove its moment

风险：修改了入睡和醒来，时间线仍把这一晚留在旧的日子；修改换了 id，历史页和时间线指向不存在的一晚；删除留下了没有详情的 Moment。前置：真实 PostgreSQL 与组件测试；历史页列出 10/27 与 10/29 两晚，10/28 为空。触发：把一晚改到一天以后；删除它；在历史页改 10/27 的入睡时间、删除 10/29（再确认一次）、补记 10/28。

必须成立：修改后 id 不变，时间线只在新的醒来时间返回它；删除后夜晚和 Moment 都不在；历史页依次发出修改、删除与补记请求，补记默认填近几晚的平均时间，落在 10/28。

禁止结果：一晚在两天都出现，或修改后换了身份；删除只点一次就生效。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 修改保留身份并移动 Moment，删除带走 Moment（真实 PostgreSQL） | Covered | `crates/db/tests/postgres/sleep.rs::correcting_or_deleting_a_night_must_move_or_remove_its_moment` |
| 历史页的修改、再确认删除与补记（组件测试，请求经 mock fetch） | Covered | `apps/palace-web/src/pages/sleep.test.tsx` 的 “corrects, deletes and fills in nights from the history” |

决策依据：D3、D5，不变量 7。

## Sleep must stay within the owner scope

风险：睡眠接口没有按 Owner 过滤，其他 Owner 能读到、修改、删除别人的夜晚，或结束别人的计时。前置：真实 PostgreSQL 与真实 Router；Owner A 有一晚和一个计时中的开始。触发：B 修改、删除 A 的一晚，用 A 的 `started_at` 结束，读取夜晚列表、计时与时间线。

必须成立：B 的写入都是未找到，A 的数据不变；B 读到的列表与时间线为空，计时为 `null`。

禁止结果：跨 Owner 的请求返回能证明对方存在的状态，或改变任何一行。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 跨 Owner 的读写都是未找到且不改变数据（真实 PostgreSQL） | Covered | `crates/db/tests/postgres/sleep.rs::sleep_must_stay_within_the_owner_scope` |
| 接口形状、状态码与 OpenAPI 合同一致，跨 Owner 为 404（真实 Router） | Covered | `crates/backend/tests/http.rs::authenticated_http_imports_preserve_scope_and_file_parity`（经 `exercise_sleep`，响应由 OpenAPI 校验器逐一核对） |

决策依据：不变量 8。

## The menu must offer sleep by the reader's clock

风险：开始睡眠在白天出现、晚上反而没有；已经记下的一晚又被补记一次；计时中菜单里还有开始或结束；页面从下午开着到晚上，菜单仍停在下午的状态；以前的日子能开始或补记。前置：前端单元测试与组件测试，浏览器时钟被固定在不同钟点。触发：在 17、18、23、4、5、14、15 点读取菜单；计时中、今天已有一晚时读取；同一页面上把时钟从 16:00 调到 21:30 后再打开菜单；查看以前的日子。

必须成立：18:00–05:00 是「开始睡眠」，05:00–15:00 且今天没有一晚是「记录睡眠」，其余没有睡眠项；计时中没有睡眠项；菜单每次被接近时重新读钟；以前的日子没有睡眠项。

禁止结果：菜单里出现「结束睡眠」；同一天提供两次补记。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 各钟点的菜单项（前端单元测试） | Covered | `apps/palace-web/src/lib/sleep.test.ts` 的 “offers starting in the evening and night, recording in the morning, nothing while running” |
| 晚上开始、早上补记、计时中与以前的日子没有睡眠项、钟点变化后重新读取（组件测试，请求经 mock fetch） | Covered | `apps/palace-web/src/pages/sleep.test.tsx` 的 “starts in the evening and ticks on a pinned dial whose stop button stays put”、“records a night never started, refusing times that do not add up”、“lets a forgotten start lapse at 15:00 and starts over in the evening”、“pins an earlier day's night above its cards, offers nothing for it, and leads to the history” |

决策依据：D1、D2、D5。

## A forgotten start must lapse at three the next afternoon

风险：忘记结束的计时一直挂在页面顶部，第二天下午还在走；或者凌晨开始的计时提前消失。前置：前端单元测试与组件测试。触发：23:46 与 01:30 开始的计时分别在次日（或当天）14:59 与 15:00 判断；16:00 打开今天的页面，晚上再开始。

必须成立：15:00 起计时作废、表盘消失，菜单像没有开始过；晚上的「开始睡眠」写入新的开始，替换作废的那个。

禁止结果：作废的计时还能被结束；15:00 前的计时被当成作废。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 作废时刻按开始那晚计算（前端单元测试） | Covered | `apps/palace-web/src/lib/sleep.test.ts` 的 “lets a start lapse at 15:00 of the day after its evening” |
| 作废后表盘消失、晚上重新开始（组件测试，请求经 mock fetch） | Covered | `apps/palace-web/src/pages/sleep.test.tsx` 的 “lets a forgotten start lapse at 15:00 and starts over in the evening” |
| 新的开始在服务端替换作废的开始（真实 PostgreSQL） | Covered | `crates/db/tests/postgres/sleep.rs::starting_must_replace_any_earlier_start` |

决策依据：D1、D4。

## Ending before six must ask once more

风险：夜里摸手机误触停止按钮，整晚记录被截断；或者早上正常醒来也要多点一次。前置：前端单元测试与组件测试。触发：在 1、5、6、11、17、18、23 点判断是否确认；23:00 点停止，先选「继续睡」再选「结束」；07:12 点停止。

必须成立：18:00 到 06:00 之间先问，问题写明现在几点、睡了多久，「继续睡」有焦点且不发请求；「结束」才结束；06:00 之后直接结束、不弹窗。

禁止结果：确认对话框的默认操作是结束；06:00 后仍要确认。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 需要确认的钟点（前端单元测试） | Covered | `apps/palace-web/src/lib/sleep.test.ts` 的 “asks before ending from the evening until 06:00” |
| 确认、默认继续睡、早上直接结束（组件测试，请求经 mock fetch） | Covered | `apps/palace-web/src/pages/sleep.test.tsx` 的 “asks before ending in the night, keeps sleeping by default, and ends when told”、“ends at once in the morning” |

决策依据：D1。

## The running dial must tick without rebuilding its stop button

风险：每秒重建表盘，停止按钮的呼吸动画每秒重新开始，看起来像在抽动。前置：组件测试，计时中。触发：等待计时数字变化。

必须成立：数字变化后，停止按钮仍是同一个元素。

禁止结果：停止按钮随每秒的数字一起被替换。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 数字更新而停止按钮元素不变（组件测试） | Covered | `apps/palace-web/src/pages/sleep.test.tsx` 的 “starts in the evening and ticks on a pinned dial whose stop button stays put” |
| 动画在真实浏览器里不重启 | Partial | 组件测试只证明元素身份不变；jsdom 不运行 CSS 动画 |

决策依据：D5。

## A typed night must land on its wake day

风险：补记时 00:30 入睡被算到前一天晚上，或 23:30 入睡被算到当天深夜，时长差出 24 小时；醒来晚于现在、或醒来早于入睡的记录被保存。前置：前端单元测试与组件测试。触发：在 10/29 早上 09:00 读入 23:30–07:05、0130–07:05、23:30–10:30、07:30–07:05、12:00–07:00、格式错误；在页面上补记并调整醒来时间。

必须成立：12:00 及以后的入睡算前一天，之前的算当天；不合理的时间给出原因、标出字段并禁用保存；保存的时间段正好落在醒来那天。

禁止结果：保存一段醒来在未来、长于 16 小时或长度为负的夜晚。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 钟点到时间段的换算与原因（前端单元测试） | Covered | `apps/palace-web/src/lib/sleep.test.ts` 的 “reads a typed night onto its wake day and names what does not add up” |
| 对话框禁用保存、给出原因、按换算结果写入（组件测试，请求经 mock fetch） | Covered | `apps/palace-web/src/pages/sleep.test.tsx` 的 “records a night never started, refusing times that do not add up” |

决策依据：D2。

## A night must be pinned to its wake day and lead to its history

风险：睡眠既置顶又出现在列表里；只有睡眠的一天显示“这一天还没有记录。”；7 晚柱把缺的一天画成零高的柱；表盘没有带来源日期，历史页回不到原来的那天。前置：组件测试；10/26 与 10/27 各有一晚，查看 10/27。触发：打开 10/27，查看表盘与列表，点击表盘。

必须成立：表盘写出时长与入睡、醒来时间；列表里没有睡眠项，也没有空白提示；7 晚柱前五天是短横、10/26 是柱、10/27 是高亮的柱；点击后进入 `/sleep?date=2026-10-27`，返回链接回到 10/27。

禁止结果：同一晚画两次；表盘在数据到达时让下面的卡片跳动。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 置顶、列表排除、7 晚柱、通往历史页与返回（组件测试，请求经 mock fetch） | Covered | `apps/palace-web/src/pages/sleep.test.tsx` 的 “pins an earlier day's night above its cards, offers nothing for it, and leads to the history” |
| 数据到达前空表盘占位，卡片不跳动 | Missing | 没有直接证据；jsdom 不排版 |

决策依据：D5。
