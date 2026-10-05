---
status: approved
date: 2026-10-05
---

# 标题从 Conversation 移到 Path，每条 Path 有自己的标题

标题不再属于 Conversation，而是每条 Path 各有一个。新建导入的标题写入第一条 Path；分支导入和追加更新都携带标题，分支可以起新名字，续写后也可以改名。编辑对话框中的标题只改当前 Path，来源仍改整个 Conversation，两者在同一个事务中保存。现有数据迁移时，把 Conversation 的标题复制给它的每条 Path，然后删除 `conversation.title`。需要以 Conversation 为单位显示名称时，取最早创建的那条 Path 的标题，不另外保存。

当前为 `approved`，已通过评审，核心测试用例见[Conversation 树与 Session Path](../../../test-cases/server/conversation/message-tree.md)与[明确目标的线性导入](../../../test-cases/server/import/linear-path-import.md)。修改[Conversation 承载树，Path 承载来源 Session](20260919-conversation-tree-and-session-paths.md)中“Conversation 拥有标题”的约定、[卡片菜单统一更新 Conversation 标题与来源](20260919-menu-action-edits-conversation-metadata.md)的编辑接口，以及[创建会话、创建分支和追加更新使用明确的导入目标](../import/20260919-explicit-branch-and-append-imports.md)的请求体与幂等摘要。前一份决策[对话在摘星阅读页阅读与管理](20261004-reading-page-owns-conversation-actions.md)与[Moment 根决策](../moment/0-moment-supertype-and-typed-details.md)、[对话 Moment 卡片字段](../moment/20261004-conversation-moment-card-fields.md)已直接按本决策改写。涉及 `crates/db` 的迁移与领域操作、`palace-server` 的导入与编辑接口及 OpenAPI、`apps/palace-web` 的阅读页、编辑与导入对话框。旧编辑接口 `PUT /api/conversations/{id}` 直接移除，不保留兼容层。

## 问题与约束

- 时间线上一张卡片对应一条 Path，同一 Conversation 的多条 Path 落在不同日期（Moment 根决策 D3）。分支往往讨论不同的事，例如同一次出行规划分成“去 Mendocino”和“改去 Point Reyes”；共用一个标题时，两张卡片在各自日期上无法区分。
- 来源是 Path 外部身份 `(owner_id, source, session_id)` 的一部分，`conversation_path.source` 通过级联外键跟随 Conversation（编辑元数据决策 D3）。来源只能整棵树一起改，不能随标题一起下放到 Path。
- 领域模型要求卡片字段从详情派生，不能形成第二份副本。

## 继承与修改

| 前序约定 | 本次决定 |
| --- | --- |
| Conversation 拥有标题、来源及消息树（树决策） | 修改：标题移到 Path，见 D1；来源与消息树仍属于 Conversation |
| “继续对话”使用 Conversation 的来源与当前 Path 的 Session ID | 继承，不变 |
| `PUT /api/conversations/{id}` 原子替换 Conversation 的标题与来源 | 修改：改为按 Path 编辑，见 D3 |
| 标题去除首尾空白后非空，UTF-8 编码不超过 1024 字节，不静默改写 | 继承；规则作用于 Path 标题 |
| 改来源时用每条 Path 的 Session ID 检查身份冲突，级联更新 Path 来源 | 继承，不变 |
| 元数据更新不刷新 Path 的 `updated_at` 或 `occurred_at`，不改变默认 Path | 继承，不变 |
| 新建导入携带标题，创建 Conversation 及第一条 Path | 修改：标题写入第一条 Path，见 D2 |
| 分支导入拒绝标题与来源 | 修改：必须携带标题，仍拒绝来源，见 D2 |
| 追加更新只接受完整 JSON、发生时间和幂等键 | 修改：同时携带标题，成功后替换该 Path 的标题，见 D2 |
| 幂等摘要包含操作类型、目标身份、原始历史与发生时间 | 修改：加入标题，见 D2 |
| 同一 Conversation 的多张 Path 卡片共用标题（Moment 根决策 D3） | 修改：每张卡片显示自己 Path 的标题，见 D4 |

## D1：标题是 Path 的字段

`conversation_path` 增加 `title text NOT NULL`，校验规则与原 `conversation.title` 相同；`conversation` 不再有标题列。

| 字段 | 归属 |
| --- | --- |
| 标题 | Path |
| 来源 | Conversation（Path 上的 `source` 是外键副本，只用于身份唯一性） |
| Session ID、末端、发生时间 | Path |
| 消息树 | Conversation |

标题与 Session 一样属于 Path，是因为它们描述的都是“那一次会话”：时间线按 Path 出现，读者在某一天看到的也是那次会话的名字。

### 迁移

下一个迁移文件按以下顺序执行，并放在同一个事务中：

1. 给 `conversation_path` 加可空的 `title`。
2. 把每条 Path 的 `title` 设为所属 Conversation 的 `title`。
3. 加上 `NOT NULL` 和与原列相同的 `CHECK`。
4. 删除 `conversation.title`。

迁移不改动任何 ID、`updated_at` 或 `occurred_at`。迁移后每条 Path 的标题都等于原 Conversation 标题，界面看到的内容与迁移前相同，直到用户编辑或导入新分支。

## D2：三种导入都携带标题

| 操作 | 标题 | 表单默认值 |
| --- | --- | --- |
| 新建导入 | 必填，写入第一条 Path | 空 |
| 分支导入 | 必填，写入新 Path | 发起分支管理时正在阅读的 Path 的标题 |
| 追加更新 | 必填，替换该 Path 的标题 | 该 Path 当前的标题 |

- 默认值由前端填入，服务端不替调用方补标题；请求缺少标题或标题不合法时整体拒绝，不创建或修改任何数据。
- 分支导入仍拒绝来源字段：新 Path 的来源必须与 Conversation 一致。
- 追加更新允许改名，是因为续写可能让话题偏离原来的标题，此时顺手改名比事后再打开编辑框更自然。标题与消息、发生时间在同一事务中提交或回滚。
- 幂等摘要加入标题：同一个幂等键携带不同标题重试会返回冲突，不会被当作同一次请求；同键同请求返回原结果，不再次写入标题。

## D3：编辑对话框改当前 Path 的标题和整个 Conversation 的来源

接口改为：

```text
PUT /api/conversations/{conversation_id}/paths/{path_id}
{ "title": string, "source": "chatgpt" | "gemini" | "grok" }
```

- `title` 替换该 Path 的标题，其他 Path 的标题不变。
- `source` 替换 Conversation 的来源，并级联到它的全部 Path；冲突检查与原决策相同。
- 两项在同一事务中保存，一次保存只有一个成功或失败结果。
- 返回 `{ "conversation_id", "path_id", "title", "source" }`。
- Path 不存在、不属于该 Conversation 或不属于当前 Owner 时，与 Conversation 不存在的响应相同，不泄露其他 Owner 的数据。

对话框仍是一个，菜单项仍叫“编辑对话”。“标题”输入框预填当前 Path 的标题，“来源”选择框预填 Conversation 的来源。同一个对话框里两个字段作用范围不同，代价是用户需要理解改来源会影响所有分支；但来源纠错几乎只在导入选错时发生，而且所有分支本来就来自同一个产品，作用于整棵树符合直觉。

## D4：显示规则

| 位置 | 显示的标题 |
| --- | --- |
| 时间线卡片 | 该卡片对应 Path 的标题 |
| 阅读页标题与页面 `<title>` | 当前 Path 的标题；在分叉处切换 Path 后随之改变 |
| 以 Conversation 为单位的列表（现有对话列表，及以后的摘星入口页） | Conversation 名称，见下 |

Conversation 名称取该 Conversation 中 `created_at, id` 最小的 Path 的标题，查询时派生，不保存。用 `created_at` 而不用 `occurred_at`，是因为追加更新会改变 `occurred_at`，名称不应随续写变动；最早的 Path 通常就是最初导入的那段对话。这条 Path 被删除后，名称改由剩余 Path 中最早的一条提供。

## 不变量

1. 每条 Path 都有一个符合标题规则的标题；Conversation 不保存标题。
2. 编辑或追加更新某条 Path 的标题时，其他 Path 的标题不变。
3. 来源修改同时作用于 Conversation 的全部 Path，并与标题在同一事务中提交或回滚。
4. 同一 Path 在时间线卡片和阅读页上显示同一个标题。
5. Conversation 名称总是等于 `created_at, id` 最小的现存 Path 的标题。
6. 同一幂等键携带不同标题时返回冲突；同键同请求的重试不改变任何 Path 的标题。
7. 迁移后每条 Path 的标题等于其所属 Conversation 原来的标题，所有 ID 与时间不变。

## 为什么不是这些替代方案

| 替代方案 | 优势 | 本次取舍 |
| --- | --- | --- |
| 保持 Conversation 共享标题 | 不改数据模型 | 分支在不同日期的卡片无法区分，违背按 Session 回看的意图 |
| Path 上加可空的标题覆盖，空时回退到 Conversation 标题 | 迁移只需加一列 | 一个名字有两处来源，编辑时用户无法判断改的是哪一个；不变量 4 需要依赖回退规则才能成立 |
| Conversation 与 Path 各保存一个标题 | 摘星列表有明确的对话名 | 两个名字都要维护，导入时还要决定改哪一个；摘星入口页尚未设计，现在不需要独立的对话名 |
| 保存 Conversation 名称列，导入时同步 | 列表查询无需取最早的 Path | 成为第二份副本，删除最早 Path 或编辑其标题时都要同步 |
| 来源编辑与标题编辑拆成两个操作 | 作用范围一目了然 | 菜单多一项低频操作；来源纠错极少，放在同一个对话框里不会造成实际误操作 |
| 分支导入由服务端复制原 Path 标题作为默认值 | 请求体不变 | 默认值属于界面行为；服务端补字段会让同一请求在不同时刻得到不同的标题，也让幂等摘要难以确定 |

## 风险与为什么不能直接改写

删除 `conversation.title` 不可逆，迁移必须先完成复制并在同一事务中校验 `NOT NULL`，失败时整体回滚。

导入回执 `conversation_import.result` 不含标题，迁移不需要改写它。但已保存的 `input_digest` 是按旧摘要规则算出的，不含标题；上线后用旧幂等键重试，即使请求完全相同也会得到冲突。幂等键由前端在每次提交时生成，重试只发生在同一次提交的几秒内，跨发布重试旧请求不是需要支持的场景，因此不为旧摘要保留兼容计算。

## 本决策未解决的问题

- **摘星入口页**：以 Conversation 为单位浏览时，除名称外是否还要展示各分支的标题，由摘星的根决策决定。
- **标题的自动生成**：导入时是否由模型根据内容建议标题，另行决定。

## 落地顺序

1. **迁移与领域操作**：按 D1 迁移；导入、分支、追加写入 Path 标题；编辑按 Path 进行。完成条件：迁移测试验证不变量 7；数据库测试覆盖不变量 2、3、6。
2. **接口与契约**：按 D2、D3 修改请求体与编辑接口，删除旧接口，更新 OpenAPI；现有对话列表按 D4 返回 Conversation 名称。完成条件：HTTP 测试覆盖缺少标题被拒绝、跨 Owner 编辑被拒绝和不变量 5。
3. **前端**：编辑与导入对话框按 D2、D3 预填；阅读页标题跟随当前 Path。完成条件：组件测试覆盖分支导入的默认标题与切换 Path 后的标题变化。

第 1、2 步必须在同一次发布中上线，因为迁移后旧接口已无法工作；从第 3 步起，界面上的分支可以有不同的标题。
