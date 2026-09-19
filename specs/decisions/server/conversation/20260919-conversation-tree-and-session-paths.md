---
status: approved
date: 2026-09-19
---

# Conversation 承载树，Path 承载来源 Session

Conversation 是主页面的一张卡片，拥有标题、来源及消息树；每个来源 Session 对应一个独立 Path。本文已由产品讨论确认，代替根决策中 Conversation 与来源 Session 一一对应的约定。

## 继承与修改

| 根决策约定 | 本次决定 |
| --- | --- |
| 一条消息一条记录，父子关系不可修改，共享前缀 | 继承 |
| Conversation 对应一个 source/session_id | 修改：一个树可以包含多个 Session，身份移至 Path |
| 原始链接来自受控来源模板 | 继承：使用 Conversation 的 source 与当前 Path 的 Session ID |
| Path 由消息末端派生，无独立记录 | 修改：Path 拥有稳定 ID、Session ID、末端和时间 |
| 不同根路径可以属于同一 Conversation | 修改：新分支必须共享至少到 assistant 的完整前缀 |

## D1：Path 的身份独立于叶子

`(owner_id, source, session_id)` 全局唯一地定位一个 Path。一个 Path 必须属于同一 Owner、同一来源的 Conversation，末端必须属于该 Conversation。多个 Path 可以共享全部消息，Path 也可以结束于另一个 Path 的内部节点。

`occurred_at` 由用户提供；`created_at`、`updated_at` 由数据库生成，更新时保持创建时间。Path 的成功更新刷新 `updated_at`；幂等重试不刷新时间。

## D2：阅读界面始终展示一个真实 Path

详情初次打开选择 `updated_at DESC, id DESC` 第一条 Path，也允许链接明确指定 Path。每个分叉点都有选择器，切换上游分叉后在符合前缀的 Path 中选更新时间最新的一条，再按它展开后续分叉，不能拼接不属于任何 Path 的消息序列。

路径在内部节点结束时允许选择“在此结束”，同一末端的多个 Session 分别可选。底部“继续对话”唯一对应当前 Path 的来源链接，替换顶部“查看原会话”。卡片按所有 Path 中最大的 `occurred_at` 排序，消息数是树的实际消息总数。

## D3：树状管理只投影 user 节点

卡片菜单包含“分支管理”和“删除对话”。管理弹窗隐藏 assistant 节点，user 节点连接最近的 user 祖先；正文按可用宽度单行省略，兼容中英文。每个 Path 的末端映射到它最后一个 user 节点，不要求该节点为叶子；同节点多个 Path 分别提供更新、删除。只有 assistant 的合法历史没有 user 可投影，其 Path 操作放在独立的“无用户消息”区域，避免隐藏可管理的数据。

## D4：删除只清理失去引用的消息

删除 Path 原子删除该 Path、相关导入凭据及没有其他 Path 引用的后缀，保留共享祖先。最后一个 Path 使用“删除对话”删除，避免空卡片。删除 Conversation 原子删除其全部 Path、消息和导入凭据。所有操作必须验证 Owner，不能通过相同 Session ID 访问其他用户数据。

## 为什么不是其他方案

| 替代方案 | 取舍 |
| --- | --- |
| 一个 Session 一张卡片 | 无法在一张卡片内管理整棵讨论树 |
| 一个叶子就是一个 Session | 无法表达相同路径或结束在内部节点的 Session |
| 相同文本自动跨 Conversation 合并 | 无法证明来源派生关系，可能错误关联独立对话 |

## 迁移与不变量

现有数据经用户确认，每个 Conversation 都只有一条线性路径。迁移保留 Conversation/Message ID，将原 Session ID 移入首个 Path，末端取最后创建的消息，发生时间取最近创建的导入记录；不推断旧多分支关系。旧操作摘要不增加兼容算法，升级前未完成的请求应重新提交。

任何对外显示的 Path 都属于该 Owner 的同一树；相同 Session 不能对应多个 Path；更新不改变 Path 身份；读取树和 Path 头使用一致数据库快照。

## 落地顺序

先迁移存储与导入规则并用真实 PostgreSQL 验证，再接入分支管理及逐节点切换。现有通用同步记录协议不扩展为消息树同步；消息编辑、跨树合并及恢复删除不在本次范围。
