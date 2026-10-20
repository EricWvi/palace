# 札记读取接口核心测试用例

当前决策：[回响页](../../../decisions/web/echo/0-jotting-wall-with-daily-recall-pile.md) D8。札记的同步、上传与对账见[札记同步](../../server/jotting/jotting-sync.md)；本文件只记录 Web 读取接口引入的承诺。接口尚未实现，所有证据均为 `Missing`。

## Jotting list must page every live jotting exactly once, newest first

风险：分页以偏移量实现，翻页途中新建或删除札记导致重复或遗漏；同一毫秒的札记顺序不稳定，跨页边界时丢失；tombstone 出现在列表中。前置：真实 PostgreSQL；索引中多于一页的札记，其中若干张 `created_at` 相同并跨过分页边界；另有若干 tombstone。触发：不带条件逐页请求 `GET /api/jottings` 直到 `next_cursor` 为 `null`；取完第一页后上传一张新札记、删除一张尚未读到的札记，再继续翻页；传入无法解析的游标。

必须成立：拼接结果按 `created_at DESC, id DESC` 排列；翻页期间未变化的札记恰好出现一次；新建的札记（`created_at` 晚于游标）不出现在后续页中；tombstone 从不出现；无法解析的游标返回 400；`limit` 默认 60，上限 200。禁止偏移量分页，禁止按路径或 commit 时间排序。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 逐页取完后每张未变化的札记恰好一次，顺序为 `created_at DESC, id DESC`（真实 PostgreSQL） | Missing | — |
| 翻页途中新建、删除不造成重复；tombstone 不出现（真实 PostgreSQL） | Missing | — |
| 坏游标返回 400，`limit` 默认 60、上限 200（真实 Router） | Missing | — |

决策依据：D8 列表与搜索，不变量 4。

## Tag and query filters must intersect and report a matching total

风险：`tag` 做了子串匹配，`#读` 命中 `#阅读`；`q` 只搜正文，漏掉标题或标签；两者同时给出时取了并集；`total` 与可翻页得到的数量不一致，筛选行显示错误的张数。前置：真实 PostgreSQL；札记分别只在标题、只在正文、只在标签中包含关键词，大小写不同；标签互为子串（`读`、`阅读`）。触发：分别以 `tag`、`q`、`tag + q` 请求并翻完全部页。

必须成立：`tag` 精确匹配；`q` 在标题、正文、标签中忽略大小写做子串匹配；同时给出时结果同时满足两者；每种条件下 `total` 等于翻页得到的札记数。禁止 `q` 依赖分词，禁止 `total` 受分页影响。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| `tag` 精确匹配，`q` 覆盖标题、正文、标签且忽略大小写（真实 PostgreSQL） | Missing | — |
| `tag + q` 取交集，`total` 与翻页结果一致（真实 PostgreSQL） | Missing | — |

决策依据：D7、D8 列表与搜索，不变量 6。

## Pending must mean the jotting file sits under inbox

风险：`pending` 按“尚未入 commit”计算，原样 commit 但仍在 `inbox/` 的札记被显示为已整理；或移出 `inbox/` 但未 commit 时被判定错误；`inbox` 的判定只比较字符串前缀，`inboxes/` 也被误判。前置：真实 Git 仓库与真实 PostgreSQL；四张札记分别处于：上传后未 commit、原样 commit 在 `inbox/`、已移到其他目录并 commit、位于名为 `inboxes/` 的目录。触发：请求 `GET /api/jottings`。

必须成立：前两张 `pending` 为 `true`，后两张为 `false`；判定以札记目录为根、以路径的第一级目录为 `inbox` 为准。禁止以 commit 状态推断 `pending`，禁止 `changes` 响应因此新增路径或 `pending` 字段。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 四种状态的 `pending` 判定正确（真实 Git 仓库与真实 PostgreSQL） | Missing | — |
| `changes` 响应形状不变（真实 Router，OpenAPI 合同） | Missing | — |

决策依据：D4、D8，不变量 5、12。

## Overview must bucket jottings by the deployment's local day

风险：每日数量按 UTC 分桶，深夜写的札记落到前一天或后一天；没有札记的日期被省略，热力图错位；标签计数包含 tombstone；标签顺序不稳定，侧栏前 8 个在刷新间跳动。前置：真实 PostgreSQL；`PALACE_TIMEZONE` 与 UTC 相差 8 小时；札记分布在本地日期边界两侧（本地 23:59 与次日 00:01）；中间有几天没有札记；若干标签数量相同；另有带标签的 tombstone。触发：以若干天前的日期为 `from` 请求 `GET /api/jottings/overview`；传入格式错误或晚于今天的 `from`。

必须成立：`daily` 从 `from` 到 `PALACE_TIMEZONE` 的今天逐日连续，空日期为 0，边界两侧的札记落在各自的本地日期；`days` 是有札记的本地日期数；`total` 与 `tags` 只计未删除的札记；`tags` 按数量降序、名称升序。格式错误的 `from` 返回 400。禁止按 UTC 日期分桶。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 按 `PALACE_TIMEZONE` 分桶，`daily` 连续且包含空日期（真实 PostgreSQL） | Missing | — |
| `total`、`days`、`tags` 排除 tombstone，`tags` 顺序稳定（真实 PostgreSQL） | Missing | — |
| 错误的 `from` 被拒绝（真实 Router） | Missing | — |

决策依据：D5、D8 概览。

## Recall must return the same pile all day and exclude today's jottings

风险：选取依赖数据库行序或进程内随机数，同一天两次请求得到不同的卡片组；今天新建的札记进入卡片组，每次新建都改变卡片组；跨过本地零点后仍返回昨天的卡片组。前置：真实 PostgreSQL；注入的时钟与 `PALACE_TIMEZONE`；候选札记多于 5 张；另有今天新建的札记与 tombstone；另一组候选少于 5 张。触发：同一天内两次请求 `GET /api/jottings/recall`，中间上传一张新札记；把时钟推进到本地次日再请求；对少于 5 张候选的库请求。

必须成立：同一天两次请求的 `jottings` 完全相同，顺序为 `SHA-256(date ‖ id)` 升序；今天新建的札记与 tombstone 从不出现；`date` 是 `PALACE_TIMEZONE` 的今天，跨过本地零点后按新日期重新计算；候选少于 5 张时全部返回。禁止为卡片组建表或写入任何状态。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 同一天、候选不变时结果与顺序完全相同，符合摘要排序（真实 PostgreSQL，注入时钟） | Missing | — |
| 今天新建的札记与 tombstone 被排除，新建不改变当天卡片组（真实 PostgreSQL） | Missing | — |
| 日期按 `PALACE_TIMEZONE` 计算，跨零点切换；候选不足时全部返回 | Missing | — |

决策依据：D3、D8 今日回响，不变量 1、2。
