---
status: implemented
date: 2026-10-05
---

# 标题从 Conversation 移到 Path，每条 Path 有自己的标题

标题不再属于 Conversation，而是每条 Path 各有一个。新建导入的标题写入第一条 Path；分支导入和追加更新都携带标题，分支可以起新名字，续写后也可以改名。编辑对话框中的标题只改当前 Path，来源仍改整个 Conversation，两者在同一个事务中保存。现有数据迁移时，Conversation 的标题被复制给它的每条 Path，然后删除 `conversation.title`。需要以 Conversation 为单位显示名称时，取最早创建的那条 Path 的标题，不另外保存。

当前为 `implemented`，核心测试用例见[Conversation 树与 Session Path](../../../test-cases/server/conversation/message-tree.md)与[明确目标的线性导入](../../../test-cases/server/import/linear-path-import.md)。修改[Conversation 承载树，Path 承载来源 Session](20260919-conversation-tree-and-session-paths.md)中“Conversation 拥有标题”的约定、[卡片菜单统一更新 Conversation 标题与来源](20260919-menu-action-edits-conversation-metadata.md)的编辑接口，以及[创建会话、创建分支和追加更新使用明确的导入目标](../import/20260919-explicit-branch-and-append-imports.md)的请求体与幂等摘要。旧编辑接口 `PUT /api/conversations/{id}` 已移除，没有兼容层。

## 问题与约束

- 时间线上一张卡片对应一条 Path，同一 Conversation 的多条 Path 落在不同日期（Moment 根决策 D3）。分支往往讨论不同的事；共用一个标题时，两张卡片在各自日期上无法区分。
- 来源是 Path 外部身份 `(owner_id, source, session_id)` 的一部分，`conversation_path.source` 通过级联外键跟随 Conversation。来源只能整棵树一起改。
- 卡片字段从详情派生，不能形成第二份副本。

## 继承与修改

| 前序约定 | 本决定 |
| --- | --- |
| Conversation 拥有标题、来源及消息树（树决策） | 修改：标题移到 Path，见 D1；来源与消息树仍属于 Conversation |
| “继续对话”使用 Conversation 的来源与当前 Path 的 Session ID | 继承，不变 |
| `PUT /api/conversations/{id}` 原子替换 Conversation 的标题与来源 | 修改：改为按 Path 编辑，见 D3 |
| 标题去除首尾空白后非空，UTF-8 编码不超过 1024 字节，不静默改写 | 继承；规则作用于 Path 标题 |
| 改来源时用每条 Path 的 Session ID 检查身份冲突，级联更新 Path 来源 | 继承，不变 |
| 元数据更新不刷新 Path 的 `updated_at` 或 `occurred_at`，不改变默认 Path | 继承，不变 |
| 新建导入携带标题，创建 Conversation 及第一条 Path | 修改：标题写入第一条 Path |
| 分支导入拒绝标题与来源 | 修改：必须携带标题，仍拒绝来源，见 D2 |
| 追加更新只接受完整 JSON、发生时间和幂等键 | 修改：同时携带标题，成功后替换该 Path 的标题，见 D2 |
| 幂等摘要包含操作类型、目标身份、原始历史与发生时间 | 修改：目标身份中包含标题，见 D2 |
| 同一 Conversation 的多张 Path 卡片共用标题（Moment 根决策 D3） | 修改：每张卡片显示自己 Path 的标题，见 D4 |

## D1：标题是 Path 的字段

`conversation_path.title` 为 `NOT NULL`，数据库检查去除首尾空白后非空，长度上限由领域校验执行；`conversation` 没有标题列。

| 字段 | 归属 |
| --- | --- |
| 标题 | Path |
| 来源 | Conversation（Path 上的 `source` 是外键副本，只用于身份唯一性） |
| Session ID、末端 | Path |
| 发生时间 | Path 对应的 Moment |
| 消息树 | Conversation |

标题与 Session 一样属于 Path，是因为它们描述的都是“那一次会话”：时间线按 Path 出现，读者在某一天看到的也是那次会话的名字。

## D2：三种导入都携带标题

| 操作 | 标题 | 表单默认值 |
| --- | --- | --- |
| 新建导入 | 必填，写入第一条 Path | 空 |
| 分支导入 | 必填，写入新 Path | 发起分支管理时正在阅读的 Path 的标题 |
| 追加更新 | 必填，替换该 Path 的标题 | 该 Path 当前的标题 |

- 默认值由前端填入，服务端不替调用方补标题；缺少标题或标题不合法时整体拒绝，不创建或修改任何数据。
- 分支导入仍拒绝来源字段：新 Path 的来源必须与 Conversation 一致。
- 追加更新允许改名，是因为续写可能让话题偏离原来的标题。标题与消息、发生时间在同一事务中提交或回滚。
- 幂等摘要包含标题：同一个幂等键携带不同标题重试返回冲突；同键同请求返回原结果，不再次写入标题。

## D3：编辑对话框改当前 Path 的标题和整个 Conversation 的来源

```text
PUT /api/conversations/{conversation_id}/paths/{path_id}/metadata
{ "title": string, "source": "chatgpt" | "gemini" | "grok" }
```

- `title` 替换该 Path 的标题，其他 Path 的标题不变；`source` 替换 Conversation 的来源，并级联到它的全部 Path，冲突检查与原决策相同。
- 两项在同一事务中保存，一次保存只有一个成功或失败结果；返回 `{ "conversation_id", "path_id", "title", "source" }`。
- Path 不存在、不属于该 Conversation 或不属于当前 Owner 时，与 Conversation 不存在的响应相同。

评审时的地址没有 `/metadata` 后缀；`PUT /api/conversations/{id}/paths/{path_id}` 已是追加更新的接口，两者不能共用同一个方法和路径，实现时加了后缀。

对话框仍是一个，菜单项叫“编辑对话”，标题预填当前 Path 的标题，来源预填 Conversation 的来源。两个字段作用范围不同，对话框说明里写明来源对所有分支生效；来源纠错几乎只在导入选错时发生，作用于整棵树符合直觉。

## D4：显示规则

| 位置 | 显示的标题 |
| --- | --- |
| 时间线卡片 | 该卡片对应 Path 的标题 |
| 阅读页标题与页面 `<title>` | 当前 Path 的标题；在分叉处切换 Path 后随之改变 |
| 以 Conversation 为单位的列表（以后的摘星入口页） | Conversation 名称，见下 |

Conversation 名称取该 Conversation 中 `created_at, id` 最小的现存 Path 的标题，查询时派生，不保存。用 `created_at` 而不用 `occurred_at`，是因为追加更新会改变 `occurred_at`，名称不应随续写变动。目前没有读取这个名称的界面或接口：原对话列表已随 Moment 根决策移除，名称在摘星入口页出现时实现。

## 不变量

1. 每条 Path 都有一个符合标题规则的标题；Conversation 不保存标题。
2. 编辑或追加更新某条 Path 的标题时，其他 Path 的标题不变。
3. 来源修改同时作用于 Conversation 的全部 Path，并与标题在同一事务中提交或回滚。
4. 同一 Path 在时间线卡片和阅读页上显示同一个标题。
5. Conversation 名称总是等于 `created_at, id` 最小的现存 Path 的标题。
6. 同一幂等键携带不同标题时返回冲突；同键同请求的重试不改变任何 Path 的标题。
7. 迁移后每条 Path 的标题等于其所属 Conversation 原来的标题，所有 ID 与时间不变。

## 为什么不是这些替代方案

| 替代方案 | 优势 | 取舍 |
| --- | --- | --- |
| 保持 Conversation 共享标题 | 不改数据模型 | 分支在不同日期的卡片无法区分 |
| Path 上加可空的标题覆盖，空时回退到 Conversation 标题 | 迁移只需加一列 | 一个名字有两处来源，编辑时无法判断改的是哪一个 |
| Conversation 与 Path 各保存一个标题 | 摘星列表有明确的对话名 | 两个名字都要维护，导入时还要决定改哪一个 |
| 保存 Conversation 名称列，导入时同步 | 列表查询无需取最早的 Path | 成为第二份副本，删除最早 Path 或编辑其标题时都要同步 |
| 来源编辑与标题编辑拆成两个操作 | 作用范围一目了然 | 菜单多一项低频操作；来源纠错极少 |
| 分支导入由服务端复制原 Path 标题作为默认值 | 请求体不变 | 默认值属于界面行为；服务端补字段让幂等摘要难以确定 |

## 风险与持久化约束

删除 `conversation.title` 不可逆。迁移在同一事务中先复制、再校验 `NOT NULL`，失败时整体回滚；开发机上 `task run:server` 会对本地数据执行它，需要旧数据时先备份。

导入回执 `conversation_import.result` 不含标题，无需改写；但已保存的 `input_digest` 按旧规则计算，不含标题，上线后用旧幂等键重试会得到冲突。幂等键由前端在每次提交时生成，重试只发生在同一次提交的几秒内，因此不为旧摘要保留兼容计算。

## 本决策未解决的问题

- **摘星入口页**：以 Conversation 为单位浏览时，除名称外是否还要展示各分支的标题，由摘星的根决策决定。
- **标题的自动生成**：导入时是否由模型根据内容建议标题，另行决定。

## 落地与验收

迁移 `0010_path_titles.sql` 把 Conversation 标题复制给每条 Path 后删除 `conversation.title`。`ImportTarget` 的三种目标都携带标题并进入幂等摘要；`update_path_metadata` 在一个事务中更新 Path 标题与 Conversation 来源；HTTP 新增 `/metadata` 接口，分支与追加接口要求标题，旧编辑接口已删除。前端编辑与导入对话框按 D2、D3 预填，阅读页标题跟随当前 Path。

领域单元测试、真实 PostgreSQL 测试与 HTTP 契约测试覆盖不变量 1–4、6、7；不变量 5 尚无读取路径，相应核心测试用例记为 Missing。
