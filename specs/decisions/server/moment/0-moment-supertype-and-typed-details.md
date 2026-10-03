---
status: approved
date: 2026-10-03
---

# 以 `moment` 表承载时刻身份与时间定位，类型详情各自建表，对话按 Path 成为时刻

新增 `moment` 表，统一保存每个 Moment 的身份、类型和时间位置；各类型的详情仍放在各自的表中，详情行与 Moment 共用同一个 `id`。对话按来源 Session 进入时间线：每条 `conversation_path` 都是一个 `kind = 'conversation'` 的 Moment，同一 Conversation 的多条 Path 在时间线上分别出现。时间线按天查询只扫描 `moment`，再按类型批量读取卡片所需的详情字段。

当前为 `approved`，已通过评审，核心测试用例见[时刻身份与时间线](../../../test-cases/server/moment/moment-timeline.md)。涉及 `crates/db` 的迁移与查询、`palace-server` 的时间线接口和 `palace-web` 的主页面。本文件是 `server/moment` 的根决策，没有前序 ADR；它修改[Conversation 承载树，Path 承载来源 Session](../conversation/20260919-conversation-tree-and-session-paths.md)中“Conversation 是主页面的一张卡片”的约定，见 D3。不承担兼容旧对话列表接口的义务。

## 问题与约束

[领域模型](../../../../docs/product/领域模型.md)要求每个 Moment 只有一个身份、提供统一的时间线定位方式，删除和同步作用于 Moment 身份，并且禁止在一个通用记录上堆可选字段。目前只有对话一种类型，但睡眠、照片、歌曲、Journal 等类型都会进入同一条时间线。

如果各类型只存在于自己的表中，时间线就要对所有类型做 `UNION ALL`：排序、分页和索引需要分散到每张表上，“一个 Moment 身份”也没有可以落地的地方。如果把所有类型放进一张 jsonb 大表，类型约束就会从数据库中消失。

## D1：`moment` 只持有身份、类型与时间位置

| 字段 | 含义 |
| --- | --- |
| `id uuid` | Moment 身份，同时也是详情行的主键 |
| `owner_id uuid` | 遵循 [Owner 与数据隔离](../owner/0-authelia-identity-and-owner-scoped-records.md) |
| `kind text` | Moment 类型的封闭枚举，第一版只有 `conversation` |
| `occurred_at timestamptz` | 时间线定位时间 |

标题、摘要、标签等卡片字段**不**进入 `moment`。领域模型要求卡片从 Moment 派生，不能成为第二份副本。跨类型的全文检索以后用可重建的派生索引实现，不在 `moment` 中冗余正文。

时长（`ended_at`）和时间精度字段第一版不加，等第一个需要它们的类型出现时再在后续 ADR 中补充。对话目前只有用户提供的单个时间点，消息本身没有时间戳，现在加上这些字段只会永远为空或取恒定值。

## D2：详情表以复合外键锁定类型

每张详情表携带一个恒定的 `kind` 列，通过 `(owner_id, id, kind)` 外键引用 `moment(owner_id, id, kind)`：

```sql
-- 仅用于说明语义，不构成实现契约
ALTER TABLE conversation_path ADD COLUMN kind text NOT NULL DEFAULT 'conversation' CHECK(kind = 'conversation');
ALTER TABLE conversation_path ADD FOREIGN KEY(owner_id, id, kind) REFERENCES moment(owner_id, id, kind);
```

这样，一条 `kind = 'sleep'` 的 Moment 不可能挂上对话详情，其他 Owner 的详情也无法引用这条 Moment。反方向的约束是“每个 Moment 恰好有一条对应类型的详情”，仅靠外键无法表达，因此由提交时检查的 deferrable constraint trigger 保证：没有详情的 Moment 不能进入正式时间线。

## D3：对话按 Path 成为 Moment

一个 Path 对应一个来源 Session，用户在时间线上回看的也是“那天的那次会话”。因此 `conversation_path.id` 就是 Moment `id`，`conversation_path.occurred_at` 迁移到 `moment.occurred_at`，不保留两份。

| 前序约定 | 本次决定 |
| --- | --- |
| Conversation 是主页面的一张卡片，按所有 Path 中最大的 `occurred_at` 排序 | 修改：时间线上一条 Path 一张卡片，各 Path 按自己的时间定位 |
| 详情可由链接明确指定 Path | 继承：打开 Moment 时以该 Path 为当前路径展示整棵树 |
| 卡片标题来自 Conversation | 继承：同一 Conversation 的多张 Path 卡片共用标题，从 Conversation 派生 |
| 删除 Path 只清理失去引用的消息；最后一个 Path 通过删除对话删除 | 继承：删除 Path 时在同一事务中删除其 Moment；删除对话时删除全部 Path 的 Moment |

大多数 Conversation 只有一条 Path，这时 Path 与 Conversation 在时间线上看不出区别。有分支的 Conversation 会出现多张卡片，各自落在其 Session 发生的那天，这正是按 Session 回看想要的效果。按导航划分，“摘星”中以 Conversation 为单位浏览知识，这不受本决策影响。

对话 Moment 落在最后一次续写的那一天：追加导入把 Moment 的 `occurred_at` 更新为本次用户提交的时间，Moment 随之移到新的日子，身份不变。这样时间线上看到的是这个 Session 最近一次活跃的时间，而且一个 Session 只出现在一天，不会因为多次续写而在多天重复出现。代价是 Session 开始那一天的时间线上不再显示它；开始时间目前也没有单独保存。

## D4：按天查询使用调用方给出的时间区间

时间线接口接收调用方按本地时区算出的 `[start, end)` 区间，在 `(owner_id, occurred_at)` 索引上查询 `moment`，按 `occurred_at, id` 排序，再按 `kind` 分组批量读取详情。服务端不保存 `local_date`，原因如下：

- 用户的“这一天”取决于当前所在的时区，在服务端写入时固化日期，会在跨时区时出现与用户感知不一致、而且无法修正的日期。
- 区间查询使用同一个时间戳索引，不需要在写入时引入时区依赖。

代价是：旅行中记录的 Moment 按浏览时所在的时区归日，而不是按发生地的时区归日。是否需要按发生地归日，留给有地点信息的类型再决定。

## 不变量

1. 每个 Moment 恰好对应一条与其 `kind` 一致的详情行，每条详情行恰好对应一个 Moment，二者 `id` 相同。
2. Moment 与详情属于同一个 Owner，跨 Owner 引用在数据库层被拒绝。
3. 时间线定位时间只保存在 `moment` 中，详情表不再保存另一份 `occurred_at`。
4. 一个 `(owner_id, source, session_id)` 对应一个 Path，因此恰好对应一个对话 Moment。
5. 删除 Path 或 Conversation 后，不留下没有详情的 Moment。
6. 对话 Moment 的 `occurred_at` 等于该 Path 最近一次成功导入（创建或追加）时用户提交的时间；幂等重试不改变它。

## 为什么不是这些替代方案

| 替代方案 | 优势 | 本次取舍 |
| --- | --- | --- |
| 各类型独立建表，时间线用 `UNION ALL` 视图 | 不增加新表，只有一种类型时最简单 | 违反不变量 1 的身份落点；跨类型排序分页需要每张表都建同构索引，类型越多查询越难维护 |
| 单表 + jsonb 详情 | 新增类型无需迁移 | 类型约束和外键脱离数据库，正是领域模型禁止的“通用记录堆可选字段” |
| `moment` 冗余标题、摘要 | 列表与检索无需 join | 卡片成为第二份副本，标题编辑需要双写；检索应交给可重建索引 |
| Conversation 作为 Moment，取最新 Path 时间 | 一段对话在时间线上只出现一次 | 分支 Session 发生在其他日子时，那一天的时间线会缺失；与按 Session 回看的意图不符 |
| 服务端保存 `local_date` | 按天查询可以走等值索引 | 写入时要固化时区，跨时区后的归日结果错误且难以修正 |

## 风险与为什么不能直接改写

Moment `id` 一旦对外可见（详情地址、未来的同步记录、搜索索引），就不能替换。迁移必须复用现有的 `conversation_path.id`，不能生成新 ID。将来若要把 Moment 改成以 Conversation 为单位，需要一份后续 ADR，并明确旧 Path Moment 地址的重定向规则。

## 本决策未解决的问题

- **时长与精度**：`ended_at` 与时间精度的具体建模，以及是否单独保存 Session 开始时间。等第二种 Moment 类型或消息时间戳出现时再决定。
- **同步**：现有通用同步记录协议不覆盖对话树。`moment` 如何参与[同步改造](../sync/0-client-timestamp-lww-and-global-seq.md)中的 revision 与领域合并，由新的 sync ADR 决定；本决策不在 `moment` 上预留同步字段。
- **全文检索**：派生索引的形态与重建流程。

## 落地顺序

1. **迁移**：建 `moment` 表；用现有 Path 的 `id`、`owner_id`、`occurred_at` 回填；给 `conversation_path` 加 `kind` 和复合外键，删除 `occurred_at`；加 deferrable 完整性触发器。完成条件：现有导入、分支、删除测试全部通过，Path ID 不变。
2. **写入路径**：导入、追加和删除在同一事务中维护 Moment。完成条件：不变量 1、5、6 有直接测试证据。
3. **时间线接口**：按区间查询 Moment 并批量取对话卡片字段。完成条件：跨天边界、同一 Conversation 多 Path，以及追加导入后 Moment 移到新日子的查询都有直接测试。
4. **前端**：主页面改为按天的 Moment 列表，以原型为参照。现有对话列表接口与页面在此阶段移除。

第 1、2 步完成后对外行为不变；从第 3、4 步起，主页面从对话列表变为按天的时间线。
