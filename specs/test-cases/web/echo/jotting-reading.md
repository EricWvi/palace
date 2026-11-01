# 札记读取接口核心测试用例

当前决策：[回响页](../../../decisions/web/echo/0-jotting-wall-with-daily-recall-pile.md) D8、[回顾按完成次数最少者优先抽取](../../../decisions/web/echo/20261101-recall-least-completed-first-from-a-stored-daily-pile.md)。札记的同步、上传与对账见[札记同步](../../server/jotting/jotting-sync.md)；本文件只记录 Web 读取接口引入的承诺。

## Jotting list must page every live jotting exactly once, newest first

风险：分页以偏移量实现，翻页途中新建或删除札记导致重复或遗漏；同一毫秒的札记顺序不稳定，跨页边界时丢失；tombstone 出现在列表中。前置：真实 PostgreSQL；索引中多于一页的札记，其中若干张 `created_at` 相同并跨过分页边界；另有若干 tombstone。触发：不带条件逐页请求 `GET /api/jottings` 直到 `next_cursor` 为 `null`；取完第一页后上传一张新札记、删除一张尚未读到的札记，再继续翻页；传入无法解析的游标。

必须成立：拼接结果按 `created_at DESC, id DESC` 排列；翻页期间未变化的札记恰好出现一次；新建的札记（`created_at` 晚于游标）不出现在后续页中；tombstone 从不出现；无法解析的游标返回 400；`limit` 默认 60，上限 200。禁止偏移量分页，禁止按路径或 commit 时间排序。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 逐页取完后每张未变化的札记恰好一次，顺序为 `created_at DESC, id DESC`（真实 PostgreSQL） | Covered | `crates/jottings/tests/jottings/reading.rs::the_list_pages_every_live_jotting_once`（真实 Git 与 PostgreSQL，四张同一时刻的札记跨过分页边界） |
| 翻页途中新建、删除不造成重复；tombstone 不出现（真实 PostgreSQL） | Covered | `crates/jottings/tests/jottings/reading.rs::the_list_pages_every_live_jotting_once`（取完第一页后上传一张、删除一张尚未读到的） |
| 坏游标返回 400，`limit` 默认 60、上限 200（真实 Router） | Covered | `crates/backend/tests/http.rs::authenticated_http_imports_preserve_scope_and_file_parity`（真实 PostgreSQL、Git 与真实 Router，经 `tests/http/jottings.rs::exercise_reading`）、`crates/db/src/jotting_read.rs::cursors_round_trip_and_reject_foreign_tokens` |

决策依据：D8 列表与搜索，不变量 4。

## Tag and query filters must intersect and report a matching total

风险：`tag` 做了子串匹配，`#读` 命中 `#阅读`；`q` 只搜正文，漏掉标题或标签；两者同时给出时取了并集；`total` 与可翻页得到的数量不一致，筛选行显示错误的张数。前置：真实 PostgreSQL；札记分别只在标题、只在正文、只在标签中包含关键词，大小写不同；标签互为子串（`读`、`阅读`）。触发：分别以 `tag`、`q`、`tag + q` 请求并翻完全部页。

必须成立：`tag` 精确匹配；`q` 在标题、正文、标签中忽略大小写做子串匹配；同时给出时结果同时满足两者；每种条件下 `total` 等于翻页得到的札记数。禁止 `q` 依赖分词，禁止 `total` 受分页影响。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| `tag` 精确匹配，`q` 覆盖标题、正文、标签且忽略大小写（真实 PostgreSQL） | Covered | `crates/jottings/tests/jottings/reading.rs::tag_and_query_filters_intersect`（含互为子串的标签与 `%` 字面量） |
| `tag + q` 取交集，`total` 与翻页结果一致（真实 PostgreSQL） | Covered | `crates/jottings/tests/jottings/reading.rs::tag_and_query_filters_intersect`（每页一张，逐页比对 `total`） |

决策依据：D7、D8 列表与搜索，不变量 6。

## Pending must mean the jotting file sits under inbox

风险：`pending` 按“尚未入 commit”计算，原样 commit 但仍在 `inbox/` 的札记被显示为已整理；或移出 `inbox/` 但未 commit 时被判定错误；`inbox` 的判定只比较字符串前缀，`inboxes/` 也被误判。前置：真实 Git 仓库与真实 PostgreSQL；四张札记分别处于：上传后未 commit、原样 commit 在 `inbox/`、已移到其他目录并 commit、位于名为 `inboxes/` 的目录。触发：请求 `GET /api/jottings`。

必须成立：前两张 `pending` 为 `true`，后两张为 `false`；判定以札记目录为根、以路径的第一级目录为 `inbox` 为准。禁止以 commit 状态推断 `pending`，禁止 `changes` 响应因此新增路径或 `pending` 字段。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 四种状态的 `pending` 判定正确（真实 Git 仓库与真实 PostgreSQL） | Covered | `crates/jottings/tests/jottings/reading.rs::pending_means_the_file_sits_under_inbox` |
| `changes` 响应形状不变（真实 Router，OpenAPI 合同） | Covered | `crates/backend/tests/http.rs::authenticated_http_imports_preserve_scope_and_file_parity`（真实 PostgreSQL、Git 与真实 Router，经 `tests/http/jottings.rs::exercise_reading`，全部真实响应按 OpenAPI 校验；`exercise_jottings` 的 `changes` 断言未改） |

决策依据：D4、D8，不变量 5、12。

## Overview must bucket jottings by the deployment's local day

风险：每日数量按 UTC 分桶，深夜写的札记落到前一天或后一天；没有札记的日期被省略，热力图错位；标签计数包含 tombstone；标签顺序不稳定，侧栏前 8 个在刷新间跳动。前置：真实 PostgreSQL；`PALACE_TIMEZONE` 与 UTC 相差 8 小时；札记分布在本地日期边界两侧（本地 23:59 与次日 00:01）；中间有几天没有札记；若干标签数量相同；另有带标签的 tombstone。触发：以若干天前的日期为 `from` 请求 `GET /api/jottings/overview`；传入格式错误或晚于今天的 `from`。

必须成立：`daily` 从 `from` 到 `PALACE_TIMEZONE` 的今天逐日连续，空日期为 0，边界两侧的札记落在各自的本地日期；`days` 是有札记的本地日期数；`total` 与 `tags` 只计未删除的札记；`tags` 按数量降序、名称升序。格式错误的 `from` 返回 400。禁止按 UTC 日期分桶。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 按 `PALACE_TIMEZONE` 分桶，`daily` 连续且包含空日期（真实 PostgreSQL） | Covered | `crates/jottings/tests/jottings/reading.rs::overview_buckets_by_the_local_day`（上海 23:59 与次日 00:01 落在 UTC 同一天） |
| `total`、`days`、`tags` 排除 tombstone，`tags` 顺序稳定（真实 PostgreSQL） | Covered | `crates/jottings/tests/jottings/reading.rs::overview_buckets_by_the_local_day` |
| 错误的 `from` 被拒绝（真实 Router） | Covered | `crates/backend/tests/http.rs::authenticated_http_imports_preserve_scope_and_file_parity`（真实 PostgreSQL、Git 与真实 Router，经 `tests/http/jottings.rs::exercise_reading`，格式错误与超过一年）、`crates/jottings/tests/jottings/reading.rs::overview_buckets_by_the_local_day`（晚于今天、一年前） |

决策依据：D5、D8 概览。

## Recall must draw the least completed jottings once a day and keep the pile

风险：每次请求都重新抽卡，完成一张后卡片组被重新洗过；并发的首次请求抽出两组卡片组；完成次数多的札记排在少的前面；今天新建、不再回顾或已删除的札记进入卡片组；修改张数配置并重启后当天的卡片组变了；跨过本地零点后仍返回昨天的卡片组。前置：真实 PostgreSQL；注入的时钟与 `PALACE_TIMEZONE`；候选札记的完成次数分属两层，最少的那一层少于设定张数；另有今天新建的、不再回顾的札记与 tombstone。触发：用两个并发请求首次获取今天的卡片组（barrier 对齐）；完成一张后再请求；改用另一个张数重新打开服务再请求；当天删除卡片组中的一张、新建一张后再请求；推进到本地次日再请求。

必须成立：并发请求只写入一组卡片组，两者返回相同；卡片组先取完次数最少的一层，不足时按日期摘要从次数多一次的一层补足，同层顺序为 `SHA-256(date ‖ id)` 升序；当天之内卡片组成员和顺序不变，删除的那张不再出现也不补抽，新建的不进入；换了张数配置不影响当天，次日按新张数抽取；`date` 是 `PALACE_TIMEZONE` 的今天。禁止今天新建、不再回顾或已删除的札记被抽中。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 次数少的先抽，同层按日期摘要排序（真实 PostgreSQL，注入时钟） | Missing | |
| 并发的首次请求只抽出一组（真实 PostgreSQL，barrier） | Missing | |
| 当天卡片组不变：完成、删除、新建与修改张数后重开服务都不改变它；次日按新张数抽取 | Missing | |
| 今天新建、不再回顾、已删除的札记不被抽中 | Missing | |
| `PALACE_JOTTING_RECALL_SIZE` 默认 5，超出 1–18 或不是整数时启动失败 | Missing | |

决策依据：20261101 回顾决策 D1、D3、D4，不变量 1、2、3、5、8。

## Recall must skip the jottings left unfinished in the previous pile

风险：忙时没看完的卡片在一轮末尾天天出现；跳过的札记被永久遗漏；“上一次”被当成“昨天”，中间隔了没打开页面的日子时没有生效。前置：真实 PostgreSQL；最少的一层只有几张；第一天的卡片组只完成了一部分；第二天没有打开回响页。触发：第三天请求卡片组；第四天再请求。

必须成立：第三天的卡片组不含第一天未完成的札记；第四天它们重新成为候选，仍在次数最少的那一层，排在次数更多的札记之前。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 跳过最近一组中未完成的札记，只跳过一次（真实 PostgreSQL，注入时钟） | Missing | |

决策依据：20261101 回顾决策 D1，不变量 4。

## Completing a recall card must count it once and only then

风险：刷新、重试或两个标签页同时点击让次数加了两次；完成不在卡片组中的札记也计数；`retire` 没有设置不再回顾，或者重复完成时丢了 `retire`；完成次数出现在响应中。前置：真实 PostgreSQL；今天的卡片组；上一组卡片组。触发：对同一张卡片并发两次 `keep`（barrier 对齐）；再以 `retire` 完成同一张；完成上一组中的卡片；完成不在卡片组中的札记、已被清理的卡片组中的札记。

必须成立：并发两次只加 1；以 `retire` 重复完成不再计数，但札记变为不再回顾；上一组中的卡片可以完成并计数；不在卡片组中或卡片组已被清理时返回 `404`；回顾接口此后对这张卡片返回 `done: true`。禁止任何响应包含完成次数。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 并发与重复完成只计数一次（真实 PostgreSQL，barrier） | Missing | |
| `retire` 在重复完成时仍设置不再回顾 | Missing | |
| 跨零点完成上一组有效；不在卡片组中返回 404（真实 Router） | Missing | |

决策依据：20261101 回顾决策 D5、D7，不变量 6。

## New, revived and re-included jottings must start at the least completed count

风险：新札记以 0 次进入，在大家都回顾过几次之后连续多天霸占卡片组；复活的札记带着删除前的次数或“不再回顾”；恢复回顾的札记保留被排除前的低次数；对账或上传因为维护回顾状态而改变行为。前置：真实 PostgreSQL；所有参与回顾的札记都已完成若干次，最少为 2；一张札记被设为不再回顾；一张完成过的札记被 Agent 删除。触发：Agent 新建札记并 commit；删除的札记被重新 commit；恢复被排除的札记；把一张从未完成过的札记完成一次。

必须成立：三者都处于次数为 2 的那一层，与次数为 2 的其他札记一起按日期摘要排序；复活的札记参与回顾；从未完成的札记完成后次数为“当前最小值 + 1”；回顾状态的写入不改变任何札记的 `server_version`。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 新建、复活、恢复回顾都落在最少的一层（真实 Git 与 PostgreSQL） | Missing | |
| 写成 tombstone 时删除回顾状态，复活后参与回顾 | Missing | |
| 回顾状态变化不发布版本，`changes` 不变（真实 PostgreSQL） | Missing | |

决策依据：20261101 回顾决策 D2，不变量 7、10。

## Recall must keep only the two latest piles

风险：卡片组表随时间无限增长；清理删掉了“上一组”，跳过规则和跨零点完成失效。前置：真实 PostgreSQL；连续多天抽出的卡片组。触发：再抽一组新的卡片组。

必须成立：表中只剩新的一组和上一组；清理与抽卡在同一事务中完成。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 抽卡后只保留最近两组（真实 PostgreSQL） | Missing | |

决策依据：20261101 回顾决策 D8，不变量 9。
