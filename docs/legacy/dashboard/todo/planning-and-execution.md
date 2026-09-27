# Todo 每日规划与执行用例

[返回 Todo](README.md)。操作在 Todo 页与 Dashboard Today 之间共享同一条 `d_todo`，集合名称来自 `d_collection`。

## 日期和执行状态

| 保存值 / 条件                             | 用户可见语义                                 | Today / 我的一天                       |
| ----------------------------------------- | -------------------------------------------- | -------------------------------------- |
| `schedule=null`                           | 尚未规划                                     | 可被选入今天                           |
| `schedule` 是过去日期，包括 `new Date(0)` | 当前未安排，可再次规划；旧日期不作为逾期展示 | 不在 Today，可被重新选择               |
| `schedule` 是今天                         | 今日安排                                     | 出现在 Today；候选列表禁用选择         |
| `schedule` 是未来普通日期                 | 已排期                                       | 不在 Today；候选列表展示日期但禁用选择 |
| `schedule` 的毫秒值为 `4000000000000`     | No Plan / 无规划                             | 不在规划候选中                         |
| `done=true`                               | 当前安排已执行                               | 保留在 Today，以划线 / 颜色区别        |
| `completed=true`                          | 已归档                                       | 不在 Today 和规划候选中                |

`isSetDate` 只把今天和未来视为“已设置”，不是检查数据库是否非空。数据库可能仍保存昨天的日期与 `done=true`；前端已把它看成可重新安排。该版没有把过期计划逐日自动延期的任务。

## T11：给单项设置今天、明天或指定日期

1. 普通集合事项菜单 Set Date / Reset Date 打开日期弹框。
2. 选择 Today、Tomorrow 或日历日期，确认后调用 `UpdateSchedule`。
3. 写 `schedule`，同时将 `done=false`；不改变累计 `d_count`。
4. 今日安排进入首页 Today；未来安排显示明日、若干日后或月日徽标。

桌面大屏还有日期徽标快捷操作：未安排且允许规划时点击可直接设为今天；今天未完成时再点击可取消。未来日期、No Plan 和今天已经 Done 的徽标不执行这个切换。

## T12：取消安排日期

对有日期且尚未 Done 的事项选择 Unset Date，写入 Unix epoch 时间，同时重置 `done=false`。结果是“允许以后再安排”，并非 No Plan。已排今天的事项从 Today 消失。

已完成今日执行的事项菜单隐藏取消日期；用户可改期，或先撤销执行再取消。接口本身是通用日期更新，UI 限制不是后端状态机约束。

## T13：暂时不让某事项进入规划候选

对当前无有效日期的普通事项选择 No Plan，写入特殊未来时间 `4000000000000`。条目仍留在集合里并展示 No Plan，但“我的一天”不再列出。

选择 Use Plan 会通过取消日期流程恢复可规划。仍可直接 Set Date 覆盖 No Plan。迁移时应保留“允许规划”和“暂不参与规划”的差异，不把该特殊值导入为 2096 年真实事件。

## T14：从首页一次规划今天

1. 点击 Dashboard Today 右上加号，打开 My Day / 我的一天。
2. 加载当前用户所有未归档、非 Inbox、非 No Plan 事项。
3. 按集合分组，组内按累计 `d_count` 从高到低排列。已安排今天或未来的事项不可选；过去日期可重新选。
4. 勾选若干条，点击 Plan。只有非空选择才发 `PlanToday`。
5. 后端批量设置当天起点，并将这些事项 `done=false`。
6. Today 按集合分组，组内按 `d_order` 降序展示；它不是候选列表的次数顺序。

此流程是“把选中项加入今天”，不是整体替换：没有选择的既有今日事项不会因此移除。取消对话框不改变已有计划。候选接口用 `schedule IS NULL OR schedule < '2096-10-02'` 排除 No Plan，阈值是旧存储约定。

## T15：开始执行、做完一次，以及撤销

Today 中尚未 Done 的事项菜单有 Start：将标题复制到剪贴板，然后新窗口打开 TimeTagger，便于在外部记录时间。它不写 Todo 状态，也不在 Dashboard 数据库建立计时记录；没有证据表明打开页面就自动启动了 TimeTagger 计时。

在 Today 或 Todo 中对今日事项 Done：`done=true`，`d_count=d_count+1`。事项留在今日卡片中，以划线等状态区分；Done 不设置 `completed`。

对已 Done 事项 Undone：`done=false`，`d_count=d_count-1`。当前 UI 按状态只展示相应动作。服务端实现未在 SQL 条件中限制旧 `done` 值，因此它不是天然幂等状态转换；不能把重复调用理解成“仍然完成一次”。

Today 上下文菜单还支持改名、设置链接、打开 / 创建草稿和看板，未 Done 时可取消今日安排。长期归档仍回到 Todo 集合页处理。Start 只在 Today 入口提供，普通集合菜单没有同样的 TimeTagger 动作。

## T16：跨日重新安排长期事项

例如周一做完一项，周二再次打开页面：

1. Today 查询仅返回数据库当天日期内且未归档的事项，周一记录不再显示。
2. 回到“我的一天”，周一 `schedule` 已是过去日期，可重新勾选。
3. 规划周二写入新日期并清除 `done`，累计次数保留。
4. 周二 Done 后累计次数再次加 1。

前端页面重新可见时检测日期变化并失效 Today 查询。当天边界同时涉及浏览器本地日期、服务端 `time.Now()` 和数据库 `CURRENT_DATE`；迁移时须核对部署时区，不能只按 UTC 日期导入。

## 迁移验收场景

| 初始情形             | 操作                     | 应保留的业务结果                            |
| -------------------- | ------------------------ | ------------------------------------------- |
| 无计划，累计 3 次    | 安排今天 → Done → Undone | 次数依次 3、4、3；事项始终未归档            |
| 昨日 Done，累计 4 次 | 今日重新安排             | `done` 清除，累计仍为 4                     |
| 已安排明天           | 打开我的一天             | 展示未来安排，但不能直接重复加入今天        |
| No Plan              | 打开我的一天 → Use Plan  | 先不在候选中，恢复后可选                    |
| 今日 Done            | Complete → Restore       | 归档 / 恢复和执行次数独立；不能自动重复加次 |

源码：[日期工具](../../../../third_party/dashboard/client/src/lib/utils.ts)、[今日列表](../../../../third_party/dashboard/client/src/components/todo/todo-list.tsx)、[计划候选](../../../../third_party/dashboard/handler/collection/ListAll.go)、[批量安排](../../../../third_party/dashboard/handler/collection/PlanToday.go)、[Todo 模型](../../../../third_party/dashboard/model/todo.go)。
