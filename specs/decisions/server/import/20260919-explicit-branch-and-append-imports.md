---
status: approved
date: 2026-09-19
---

# 创建会话、创建分支和追加更新使用明确的导入目标

继承线性路径导入根决策的原文保存、统一校验、事务原子性与幂等；修改同 Session 自动合并规则，配合[Conversation 树与 Session Path](../conversation/20260919-conversation-tree-and-session-paths.md)实现独立 Path。

## D1：三种操作分别表达意图

主页面导入携带标题、来源、Session ID、完整 JSON、发生时间和幂等键，创建 Conversation 及第一条 Path；重复 Session 拒绝。分支导入通过 URL 的 Conversation ID 定位，正文只接受 Session ID、完整 JSON、发生时间和幂等键，拒绝标题与来源。更新通过 Conversation ID/Path ID 定位，正文只接受完整 JSON、发生时间及幂等键，禁止更换 Session ID。

更新弹窗回填 Path 原 `occurred_at`，允许修改；时间与消息在同一事务提交或回滚。文件输入先读为原始文本后使用相同领域解析器，不规范化内容。

## D2：新建共享 assistant 前缀，更新保留完整历史

新 Path 必须与已有 Path 从根开始共享至少到一个 assistant 节点的完整前缀，按 `role/content` 严格匹配；完全不重叠或仅共享 user 节点时拒绝。相同完整路径使用不同 Session ID 可合法创建分支。

更新要求新输入以该 Path 全部历史节点为前缀；只允许追加或提交完全相同的 JSON，不得修改、截短、重排历史。已被其他 Path 创建的相同后缀可以复用。并发更新在数据库内串行检查最新末端，过期历史不能覆盖已经追加的内容。

## D3：幂等键绑定操作及目标

摘要包含操作类型、目标身份、原始历史与发生时间；同键同请求返回原结果，不刷新时间，同键不同请求冲突。身份检查、消息复用、Path 修改与回执写入必须同事务。数据库唯一约束兜底跨 Conversation 的 Session 唯一性。

## 验收与后果

真实 PostgreSQL 验证前缀复用、无前缀拒绝、追加限制、并发重复 Session、失败回滚、时间和删除；HTTP 验证禁用字段不能越过界面直接提交、Owner 隔离、Path 路由和链接。

不保留旧“同 Session 再次导入自动合并”的入口。已有 Path 更新必须走更新 API；相同内容不能用于跨树自动判定关系。
