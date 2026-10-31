# 日课与工夫

时刻页上的两种记录，彼此不关联。**日课**是每天要做的功课，只记一天做了几次；**工夫**是一段花在某件事上的时间，带类型、开始时间和时长。决策见[日课与工夫 ADR](../../../specs/decisions/web/habits/0-habits-as-seals-and-activities-as-moments.md)，交互原型见 [`design/2.3-moment-habit.html`](../../../design/2.3-moment-habit.html)。

## 日课

- **印章**：每门日课是日期标题同一行右侧的一个单字方印，窄屏时换到标题下方。没做是细框淡字；做了一部分按次数比例从底部填浅朱色；做完是满朱色。悬停显示全名和 `n/次数`。
- **打卡**：点一下加一次，满了再点归零。以前的日子也能点；历史页的格子同样可以点。
- **编排**：日期旁 `···` 菜单的「编排日课」可以改名称、印章字（默认取名称第一个字）和每天次数（1 到 10 次），可以停用，也可以新增。停用的日课不再出现在每天的页面上，不能再打卡，记录保留；停用不可撤销。
- **改每天次数**不改写已有打卡；超过新次数的那天显示为做完。

## 工夫

- **记录**：`···` 菜单的「记录工夫」或快捷键 `g`。类型是一排可选的词，最近用过的排在前面，「＋」原地新建类型（同名类型不会重复建）。开始填 `07:30`，时长填分钟数或 `1:30`，备注可空。
- **默认值**：时长取这个类型近 30 天最近 10 次的平均值（取整到 5 分钟，没有记录时为 30）；今天的开始时间默认“刚刚结束”，以前的日子默认 07:00。
- **校验**：结束晚于现在、时长超过 12 小时或格式不对时，对话框写出原因并禁用保存。
- **时间线**：工夫按开始时间归日，卡片是 `[ 工夫 ] 跑步`，下一行 `07:05 – 07:40 · 35 分钟 · 备注`。点标题打开修改对话框，可以删除（再点一次「确认删除」）。

## 历史页 `/habits?date=…`

从菜单「日课与工夫」进入，返回链接回到来源日期。

- **日课**：9 周矩阵，从 8 周前的周一到今天，每门一行、每天一格，填法与印章相同，点格子打卡。右侧是近 30 天完成率（从创建那天起算）。窗口内有打卡的已停用日课以淡色只读显示。
- **工夫**：每种类型一行，按近 30 天总时长排序。左侧是“n 次 · 共 h:mm”，右侧是每日时长的圆滑曲线（单调三次插值，不冲出相邻两天的值），没做的那天落到 0。每行按自己的峰值缩放并标出峰值；悬停时竖线穿过所有行，提示框写出那天这一行的时长。

## 接口

| 方法与路径 | 作用 |
| --- | --- |
| `GET /api/habits?from=&to=` | `{ habits, checks }`；本地日 `YYYY-MM-DD` 含两端，跨度不超过 100 天 |
| `POST /api/habits` | `{ name, mark, times_per_day }`，返回 201 |
| `PUT /api/habits/{id}` | 修改名称、印章字、每天次数；已停用的是 404 |
| `POST /api/habits/{id}/retire` | 停用，返回 204，可重复 |
| `PUT /api/habits/{id}/checks/{day}` | `{ count }` 设定这天的次数，0 清除；超过每天次数是 400 |
| `GET /api/activity-types` | 全部工夫类型，按创建顺序 |
| `POST /api/activity-types` | `{ name }`，新建返回 201，同名已存在返回 200 |
| `GET /api/activities?start=&end=` | 开始时间在 `[start, end)` 内的工夫；区间不超过 400 天 |
| `POST /api/activities` | `{ type_id, started_at, minutes, note }`，返回 201 |
| `PUT /api/activities/{id}` | 修改，身份不变 |
| `DELETE /api/activities/{id}` | 连同 Moment 删除，返回 204 |

时间是 epoch 毫秒，时长 1 到 720 分钟，备注不超过 1024 字节，空白即没有备注。时间线返回 `{ "kind": "activity", "id", "occurred_at", "type_id", "type_name", "minutes", "note" }`。其他 Owner 的日课、类型和工夫都返回 404。字段的完整定义见 [OpenAPI](../../../contracts/openapi.json)。

## 验证

- 真实 PostgreSQL：`crates/db/tests/postgres/habits.rs` 与 `crates/db/tests/postgres/activities.rs`。
- HTTP 契约：`crates/backend/tests/http/habits.rs`。
- 前端：`apps/palace-web/src/lib/habits.test.ts`、`apps/palace-web/src/lib/activities.test.ts` 与 `apps/palace-web/src/pages/habits.test.tsx`。
