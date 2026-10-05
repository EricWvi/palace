---
status: implemented
date: 2026-10-04
---

# 对话 Moment 卡片展示来源、Path 消息数与开头两条消息的摘录

时间线上一张对话卡片对应一条 Path，展示该 Path 的标题、Conversation 的来源、该 Path 的消息数，以及该 Path 开头两条消息的纯文本摘录。时间线接口在按天查询 Moment 后，按 `kind` 批量读取这些字段；摘录由服务端截取，不在任何表中冗余保存。

当前为 `implemented`，核心测试用例见[时刻身份与时间线](../../../test-cases/server/moment/moment-timeline.md#conversation-cards-must-summarize-their-own-path)。继承[以 `moment` 表承载时刻身份与时间定位](0-moment-supertype-and-typed-details.md)的身份、时间定位与查询方式，具体化其时间线接口的对话卡片字段；标题归属见[标题从 Conversation 移到 Path](../conversation/20261005-title-belongs-to-path.md)。卡片的打开方式见[Web 导航根决策](../../web/navigation/0-text-nav-day-routes-and-moment-opening.md) D4，视觉参照为 `docs/design/1-prototype.html`。

## 继承与修改

| 前序约定 | 本决定 |
| --- | --- |
| `moment` 只持有身份、类型与时间位置，卡片字段不进入 `moment` | 继承，不变；卡片字段在查询时从详情派生 |
| 时间线按 `occurred_at, id` 排序，再按 `kind` 批量读取详情 | 继承，不变 |
| 同一 Conversation 的多张 Path 卡片共用标题，从 Conversation 派生 | 修改：标题属于 Path，每张卡片显示自己 Path 的标题；来源仍从 Conversation 派生 |
| 卡片上的消息数是整棵树的实际消息总数（Conversation 树与 Path 决策 D2） | 修改：时间线卡片对应 Path，消息数改为该 Path 的消息数，见 D2 |

## D1：卡片字段

`GET /api/timeline` 的每一项带 `kind` 标签，对话项的字段如下：

| 字段 | 来源 | 说明 |
| --- | --- | --- |
| `id` | Moment | 等于 Path ID |
| `kind` | Moment | `conversation` |
| `occurred_at` | Moment | 时间线定位时间 |
| `conversation_id` | Path | 打开阅读页需要 |
| `title` | Path | |
| `source` | Conversation | 界面显示为产品名称 |
| `message_count` | Path | 从根到 Path 末端的消息数 |
| `excerpt` | Path | 开头至多两条消息，见 D3 |

卡片在界面上显示为 `[ 对话 ] 标题`，下一行 `来源 · N 条消息`，再下面是摘录。

## D2：消息数按 Path 计算

“消息数是树的实际消息总数”是为会话收藏页的 Conversation 卡片定的，一张卡片代表整棵树。时间线上一张卡片代表一条 Path，不同 Path 可能出现在不同日期；如果每张卡片都显示整棵树的总数，数字与点开后看到的内容对不上。改为 Path 的消息数后，卡片上的数字等于从这张卡片打开阅读页时元信息行显示的数字。

## D3：摘录是开头两条消息的纯文本前缀

- 摘录取 Path 从根开始的前两条消息，每条包含 `role` 和 `text`；Path 只有一条消息时只返回一条。
- `text` 是消息按 GFM（表格、删除线、任务列表）解析后的可见文字：不保留 Markdown 标记和原始 HTML，块之间以空格分隔，空白折叠为单个空格；按 Unicode 标量截取前 120 个字符，被截断时去掉末尾空白再加 `…`。
- 去除标记在服务端完成，使所有客户端看到一致的摘录，也避免为了显示几十个字下发整条长消息。查询沿 Path 只携带消息 ID 向根回溯，只读取最靠近根的两条消息正文。
- 界面以“你：…”“来源名称：…”两行展示，每行超出宽度时省略。

选择开头而不是末尾两条消息，是因为开头最能说明这段对话在讨论什么；追加导入只延长 Path，不改变摘录。

## 不变量

1. 卡片的 `message_count` 等于以该 Path 打开阅读页时显示的消息数。
2. 卡片的标题始终等于该 Path 的当前标题，来源始终等于所属 Conversation 的当前来源。
3. 摘录只来自该 Path 的消息，不包含其他 Path 独有的消息。
4. 卡片字段不在 `moment` 或其他表中冗余保存。

## 为什么不是这些替代方案

| 替代方案 | 优势 | 取舍 |
| --- | --- | --- |
| 沿用整棵树的消息总数 | 与旧卡片一致 | 时间线卡片代表 Path，总数与打开后看到的内容不一致 |
| 摘录取最后两条消息 | 反映最近的进展 | 追加导入会不断改变卡片内容；结尾常是寒暄，不说明主题 |
| 返回完整消息由前端截取 | 服务端无需处理 Markdown | 长消息会让时间线响应体不可控，各客户端的截取结果也可能不同 |
| 在详情表中保存摘录 | 查询更快 | 成为第二份副本，标题与消息变化后需要同步维护 |

## 本决策未解决的问题

- **由模型生成的摘要**：产品方向提到“简短摘要”，目前用开头两条消息代替，是否引入生成摘要另行决定。
- **Moment 时间的显示**：卡片暂不显示时间，留给 Web 导航根决策的后续决定。

## 落地与验收

`palace-domain` 的 `excerpt` 用 `pulldown-cmark` 提取可见文字并截断；`palace-db` 的时间线查询以两条固定查询批量读取卡片字段与摘录；OpenAPI 中 `Moment` 是以 `kind` 区分的联合类型。前端 `TimelinePage` 按 D1 渲染卡片。

领域单元测试覆盖 Markdown 去除、空白折叠和中英文截断；真实 PostgreSQL 测试覆盖同一 Conversation 多 Path、内部末端 Path、单条消息 Path 及编辑后的标题与来源；组件测试覆盖两行摘录的渲染。
