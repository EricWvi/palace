# Glossary

## 对话导入

以下术语对应已经批准的对话存储与导入根决策。

**来源网站（Source）**：原始对话所在的 Web Chat 产品。第一版只包括 ChatGPT、Gemini 和 Grok。

**来源会话（Source Session）**：由来源网站与该网站的 `session_id` 共同标识的原始对话；`session_id` 只在所属来源网站内解释。

**对话（Conversation）**：Palace 中以一张卡片呈现的知识记录，拥有标题、来源网站及一棵消息树，可以包含多个来源会话。

**消息（Message）**：对话中的最小内容单元，由 `user` 或 `assistant` 角色及原始 Markdown 内容构成。

**对话树（Conversation Tree）**：同一对话内由消息父子关系形成的有根树；共享前缀只保存一次，一个消息拥有多个子消息时形成分叉。

**对话路径（Conversation Path）**：一个来源会话在对话树中对应的完整消息路径，具有独立身份、Session ID 和时间信息。末端不必是树的叶子；不同 Path 可以暂时具有完全相同的消息序列。

**导入（Import）**：用户提交对话历史，由系统原子创建 Conversation 及首条 Path、创建分支或更新已有 Path 的操作。创建与分支提交完整历史；更新提交完整历史（追加更新）或只提交新增消息（增量更新）。

**追加更新（Full Update）**：提交该 Path 完整历史加新消息的更新；已有历史必须原样作为前缀。

**增量更新（Delta Update）**：只提交新增消息的更新；新增消息接在用户所见的 Path 末端之后，已有历史不比对、不改变，因此 Path 前段可能与来源当前版本不同。

**分支（Branch）**：同一 Conversation 中与已有 Path 共享从根到至少一条 assistant 消息的完整前缀、具有独立来源 Session ID 的 Path。

**轮次（Turn）**：界面可按需要从连续消息派生的展示分组，不是第一版持久化身份或导入边界。

## 所有权与身份

以下术语对应已经批准的 Owner 根决策。

**Owner**：Palace 内稳定的数据所有者。业务记录通过 `owner_id` 归属于一个 Owner，Owner 身份不随 email 变化。

**Authelia Identity**：由 Authelia OIDC 的 `issuer` 与 `subject` 共同标识的外部认证身份，用于定位 Palace Owner。

**Owner Email**：Authelia 为已认证身份提供的当前 email，供显示、联系和冲突诊断，并维护与 Owner 的映射；它不是业务主键或稳定认证身份。

**Owner Scope**：一次已认证请求只能访问的 Owner 数据范围。第一版一个请求只拥有一个 Owner Scope。

## 会话

以下术语对应已经批准的长期 Palace Session 后续决策。

**Palace Session**：Palace 在完成一次 Authelia OIDC 认证后建立的服务端会话，以可撤销的 opaque cookie 恢复 Owner Scope；它不是 Authelia Session，也不是 OIDC token。

**身份复核（Identity Recheck）**：Palace 使用 Authelia OIDC refresh/UserInfo 能力重新确认 session 所绑定的 `(issuer, subject)` 与当前 email，成功后才能继续延长无需交互登录的使用时间。

## 时刻

以下术语对应已经批准的 Moment 根决策。

**Moment（时刻）**：某一天里用户希望保存、回看或继续处理的一条记录，拥有唯一身份、类型和时间线定位时间；类型专属内容保存在对应的详情中。

**Moment 类型（Moment Kind）**：Moment 的封闭分类，决定详情结构与卡片摘要；第一版只有对话。

**对话 Moment**：一条 Conversation Path 在时间线上的身份，与该 Path 共用 ID；同一 Conversation 的多个 Path 分别是不同的 Moment。

## 媒体

以下术语对应已经批准的媒体文件根决策。

**媒体文件（Media）**：Owner 上传的一个不可修改的文件，内容保存在对象存储中，由 Palace 生成的稳定 ID 标识；正文和接口只通过这个 ID 引用它，不使用对象 key 或存储地址。

## 标注

以下术语对应已经批准的摘星标注决策。

**标注（Annotation）**：读者在一篇文章正文或一条对话消息上留下的痕迹，由锚点、可选的样式（四色高亮、划线、波浪线）和可选的批注组成，二者至少有一个；属于标注对象的 Owner。

**锚点（Anchor）**：标注指向的文字，记为渲染后文字中的起止位置加被选中的原文（`quote`）；创建后不变。

**批注（Note）**：附在标注上的一段纯文本，宽屏时显示在正文右侧的页边栏。

## 札记

以下术语对应已经批准的札记根决策。

**札记（Jotting）**：标题可选的短文，以带 frontmatter 的 Markdown 文件保存在 Git 仓库中，由 frontmatter 中的 UUIDv7 `id` 标识；移动或改名文件不改变身份。与产品文档中作为 Moment 摘要视图的“卡片（Card）”无关。

**已索引 commit**：服务端最近一次成功对账的 commit；同步出去的札记状态总是它的树，加上尚未入 commit 的上传札记。

**上传札记**：移动端离线新建、由服务端写入 `inbox/` 的札记；在 Agent commit 之前已经进入索引并同步。
