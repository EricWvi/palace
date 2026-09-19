# Glossary

## 对话导入

以下术语对应已经批准的对话存储与导入根决策。

**来源网站（Source）**：原始对话所在的 Web Chat 产品。第一版只包括 ChatGPT、Gemini 和 Grok。

**来源会话（Source Session）**：由来源网站与该网站的 `session_id` 共同标识的原始对话；`session_id` 只在所属来源网站内解释。

**对话（Conversation）**：Palace 中以一张卡片呈现的知识记录，拥有标题、来源网站及一棵消息树，可以包含多个来源会话。

**消息（Message）**：对话中的最小内容单元，由 `user` 或 `assistant` 角色及原始 Markdown 内容构成。

**对话树（Conversation Tree）**：同一对话内由消息父子关系形成的有根树；共享前缀只保存一次，一个消息拥有多个子消息时形成分叉。

**对话路径（Conversation Path）**：一个来源会话在对话树中对应的完整消息路径，具有独立身份、Session ID 和时间信息。末端不必是树的叶子；不同 Path 可以暂时具有完全相同的消息序列。

**导入（Import）**：用户提交完整对话历史，由系统原子创建 Conversation 及首条 Path、创建分支或追加更新已有 Path 的操作。

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
