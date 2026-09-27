# Dashboard 与 Journal 功能基线

本目录记录 Eric 已部署旧项目的功能，供 Palace 融合 Dashboard 与 Journal 时核对。重点是用户能做什么、操作改变什么数据，以及跨页面的关系；这里不决定 Palace 的页面布局或新数据库结构。

## 版本与证据

- 源码：[`third_party/dashboard`](../../third_party/dashboard)，当前检出的发布 tag **`v2.17.3`**，commit **`08aac0cc2ba966b5b1e241affaa808420e91b7de`**。
- 整理日期：2026-09-27。仅检查该 HEAD 的工作树，没有读取 tag 后续开发版本。
- 界面：[截图索引](screenshots/README.md)，共 9 张，覆盖 Dashboard、Todo、Journey 三个子页与分享卡、Echoes、Bookmark、Journal。
- 行为依据：页面和组件 → 前端 hooks → Go handler → model / migration。截图用于确认呈现，隐藏菜单、状态变化、字段含义以该发布版代码补全。
- 本次为静态梳理，未连接部署数据库、未运行旧服务。文中“已实现”表示发布版存在调用链，不等于已经在线逐项验收；限制与可疑边界单独列明。

旧项目通过 `third_party/dashboard` Git submodule 跟踪，固定在上述 tag 对应的 commit。新检出 Palace 后，执行 `git submodule update --init third_party/dashboard` 获取该版本；不要使用 `--remote` 更新到后续开发代码。

## 阅读入口

| 页面 / 能力      | 文档                                                       | 主要问题                                   |
| ---------------- | ---------------------------------------------------------- | ------------------------------------------ |
| 页面与表总览     | [数据关系](data/README.md)                                 | 每页读写哪些表，哪些 ID 藏在 JSON 中       |
| Dashboard 首页   | [今日与速记](dashboard/overview/README.md)                 | 今日安排、Quick Note、个人资料与外部入口   |
| Todo             | [功能总览](dashboard/todo/README.md)                       | Inbox、集合、任务与两种完成语义            |
| Todo 组织管理    | [集合与任务用例](dashboard/todo/collections-and-tasks.md)  | 收集、整理、排序、归档、恢复和删除         |
| Todo 每日执行    | [规划与执行用例](dashboard/todo/planning-and-execution.md) | 日期、No Plan、今日完成与累计次数          |
| Todo 工作材料    | [草稿与看板用例](dashboard/todo/drafts-and-kanban.md)      | 富文本材料与任务内部看板                   |
| Journey          | [功能总览](dashboard/journey/README.md)                    | 待体验、进行中、已完成与放弃               |
| Journey 生命周期 | [体验与回顾用例](dashboard/journey/use-cases.md)           | 进度、评分、摘录、感想、再次体验和分享     |
| Echoes           | [周期回顾](dashboard/echoes/README.md)                     | 周回顾、年度问答、十年问答                 |
| Echoes 问题内容  | [问题清单](dashboard/echoes/questions.md)                  | 问题编号与旧答案的对应关系                 |
| Bookmark         | [网址与速查笔记](dashboard/bookmark/README.md)             | Domain、What、How 与 cheatsheet            |
| Blog             | [博客管理](dashboard/blog/README.md)                       | 发布版额外存在的桌面页面，无对应截图       |
| Journal          | [手记总览](journal/README.md)                              | 时间流、正文、元信息和媒体                 |
| Journal 写作     | [写作与分享用例](journal/writing-and-sharing.md)           | 新建、编辑、收藏、标签地点、删除和图片分享 |
| Journal 回顾     | [检索与回顾用例](journal/browsing-and-recall.md)           | 搜索、日期、日历、统计、历史今天与随机回顾 |
| 通用编辑         | [富文本与版本历史](shared/editor-and-history.md)           | 自动保存、显式保存、放弃修改、历史恢复     |
| 通用附件         | [媒体](shared/media.md)                                    | 上传、显示、存储与迁移依赖                 |
| 应用基础         | [身份、导航与集成](shared/application.md)                  | 账户、语言、跨端刷新与外部搜索             |
| Palace 融合      | [功能迁移核对](migration/README.md)                        | 迁移验收场景、数据保留和未决边界           |

## 容易混淆的概念

| 旧名称                      | 实际含义                                                         |
| --------------------------- | ---------------------------------------------------------------- |
| Dashboard                   | 既是整个主应用的名称，也是主应用中聚合“今日 + 速记”的首页        |
| Journey / Watch             | 统一管理电影、剧集、纪录片、书籍、游戏、漫画；不只是视频播放     |
| Todo `done`                 | 一次今日执行完成，可撤销；不会从活动集合永久移除                 |
| Todo `completed` / Complete | 归档事项；恢复后仍是同一条 Todo                                  |
| 用户命名的 Archive 集合     | 普通 `d_collection` 行，与 Todo 的 `completed` 状态无内建联系    |
| Draft                       | 大多数场景是 `d_tiptap` 正文 ID，不表示“尚未发布”                |
| Todo Kanban                 | 保存在 `d_tiptap.content` 的看板 JSON，不是独立任务表            |
| Bookmark 页面               | `d_bookmark` 网址 / 速查笔记，与 Journal 收藏不同                |
| Journal Bookmark            | `d_entry.bookmark` 布尔标记，不创建 `d_bookmark`                 |
| Echoes                      | 周期性回顾和固定问答，不是 Journal 的别名                        |
| 分享                        | Journey / Journal 生成 PNG；不能据此推断有公开分享链接或匿名访问 |

文档中的用例 ID 是迁移核对索引，不代表 Palace 已实现。旧行为与建议分开记录；已知实现瑕疵不自动成为 Palace 必须复制的规则。
