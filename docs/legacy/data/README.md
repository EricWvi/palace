# 页面与数据库关系

[返回功能基线](../README.md)。旧项目使用 PostgreSQL，业务表均以 `d_` 开头。下面写的是发布版实际存储结构，不是 Palace 目标模型。

## 页面读写矩阵

“正文 / 媒体”列表示只有打开相应正文、上传或显示附件时才会涉及；并非进入每个列表都会查询全部依赖。身份 `d_user` 是所有页面的公共前提。

| 页面 / 子功能               | 直接业务表                     | 正文 / 媒体                            | 主要写入                                       |
| --------------------------- | ------------------------------ | -------------------------------------- | ---------------------------------------------- |
| Dashboard 资料区            | `d_user`                       | `d_media`（头像）                      | 名称、语言、头像、RSS / 邮箱凭据               |
| Dashboard Today             | `d_todo`、`d_collection`       | `d_tiptap`、`d_media`                  | 安排日期、当天完成、撤销、标题和链接           |
| Dashboard Quick Note        | `d_quick_note`                 | `d_tiptap`、`d_media`                  | 新建、改名、置底、删除、正文编辑               |
| Todo 集合与事项             | `d_collection`、`d_todo`       | `d_tiptap`、`d_media`                  | 集合、排序、难度、计划、完成、归档、草稿和看板 |
| Journey Watching            | `d_watch`                      | `d_tiptap`、`d_media`                  | 进度、时间点、感想、摘录、完成 / 放弃          |
| Journey To Watch            | `d_watch`；开始时新增 `d_todo` | `d_tiptap`、`d_media`                  | 待体验资料、置顶、开始、删除                   |
| Journey Watched / Dropped   | `d_watch`                      | `d_tiptap`、`d_media`；分享读 `d_user` | 评分、日期、感想、再次体验 / 恢复              |
| Echoes Week / Year / Decade | `d_echo`                       | `d_tiptap`、`d_media`                  | 创建周期记录、正文、周重点标记                 |
| Bookmark                    | `d_bookmark`、`d_tag`          | `d_tiptap`、`d_media`（速查笔记）      | URL、分类、标签、点击次数、正文                |
| Blog                        | `d_blog`、`d_tag`              | `d_tiptap`、`d_media`                  | 文章元信息、正文、发布 / 归档状态              |
| Journal                     | `d_entry`、`d_tag`             | `d_tiptap`、`d_media`；分享读 `d_user` | 手记、检索文本、字数、收藏、地点标签、回顾计数 |

## 表目录

| 表             | 保存的业务对象                   | 迁移必须理解的字段                                                                                                       |
| -------------- | -------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| `d_user`       | 用户和个人配置                   | `email` 唯一，`username`、`avatar`、`language`；`rss_token`、`email_token` 加密存储，`email_feed` 是邮箱地址             |
| `d_collection` | Todo 集合                        | `name`；创建时间决定集合默认次序；Inbox ID 0 是前端虚拟集合                                                              |
| `d_todo`       | 一个可反复规划的事项             | `title`、`collection_id`、`completed`、`difficulty`、`d_order`、`link`、`draft`、`kanban`、`schedule`、`done`、`d_count` |
| `d_quick_note` | 首页速记的标题与排序             | `title`、`draft`、`d_order`                                                                                              |
| `d_tiptap`     | 可编辑 JSON 文档                 | `content`、`ts`、`history`；既装富文本，也装看板，不能统一当作 Tiptap AST                                                |
| `d_watch`      | 一次作品体验记录                 | `title`、`w_type`、`status`、作品 `year`、整数 `rate`、`author`、`payload`；`created_at` 会被开始 / 完成 / 置顶操作改写  |
| `d_echo`       | 一个周期回顾或一道周期问题的答案 | `e_type`、`year`、`sub`、`draft`、`mark`；`sub` 在周类型中是周号，在年 / 十年类型中是问题编号                            |
| `d_bookmark`   | 网址或速查笔记                   | `url`、`title`、`click`、`domain`、`payload`；`domain` 是知识领域，不是网址域名                                          |
| `d_tag`        | 可复用的标签选项                 | `name`、`t_group`；实际记录关联存在业务 `payload` 中，不是标签关联表                                                     |
| `d_blog`       | 博客文章元信息                   | `title`、`visibility`、`draft`、`payload`                                                                                |
| `d_entry`      | Journal 手记元信息与检索摘要     | `draft`、`visibility`、`payload`、`word_count`、`raw_text`、`bookmark`、`review_count`                                   |
| `d_media`      | 上传文件的访问映射               | `creator_id`、UUID `link`、对象存储 `key`、`presigned_url`、`last_presigned_time`                                        |

业务模型大多嵌入 `MetaField`：`id`、`creator_id`、`created_at`、`updated_at`、`deleted_at`。User / Media 也带创建、更新、软删除字段。一般删除是 GORM 软删除；界面没有通用回收站。不要把“恢复归档 Todo”理解成恢复软删除数据。

## 逻辑关联与 JSON

以下是代码使用的逻辑关联，不表示所有关联都受数据库外键约束保护。

| 来源字段                                                              | 目标 / 结构                                                                               | 功能意义                                              |
| --------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- | ----------------------------------------------------- |
| 各业务表 `creator_id`                                                 | `d_user.id`                                                                               | 用户隔离                                              |
| `d_todo.collection_id`                                                | `d_collection.id`，或特殊值 0                                                             | 集合归属 / Inbox                                      |
| `d_todo.draft`                                                        | `d_tiptap.id`                                                                             | 事项材料正文                                          |
| `d_todo.kanban`                                                       | `d_tiptap.id`                                                                             | 事项内部看板                                          |
| `d_quick_note.draft`、`d_echo.draft`、`d_blog.draft`、`d_entry.draft` | `d_tiptap.id`                                                                             | 各类正文                                              |
| `d_watch.payload`                                                     | `img`、`link`、`measure`、`range`、`progress`、`epoch`、`checkpoints`、`review`、`quotes` | 封面、继续链接、进度、轮次和文档引用                  |
| `d_watch.payload.review / quotes`                                     | `d_tiptap.id`                                                                             | 感想 / 摘录；再次体验会复制引用，可能与旧记录共享正文 |
| `d_bookmark.payload`                                                  | `whats: string[]`、`hows: string[]`、`draft: number`                                      | 两类标签和速查正文                                    |
| `d_blog.payload`                                                      | `whats: string[]`、`hows: string[]`                                                       | 两类标签                                              |
| `d_entry.payload`                                                     | `tags: string[]`、`location: string[]`                                                    | 普通标签与最多三级地点                                |
| 正文图片 / 视频 `src`，头像、封面                                     | `/api/m/<UUID>` 等 URL → `d_media.link`                                                   | 文件间接引用；不是媒体 ID 外键列表                    |
| `d_tiptap.history`                                                    | `[{time, content}, ...]`                                                                  | 文档历史；内容里也可能有附件引用                      |

`d_tag.t_group=dashboard` 使用 `what:`、`how:` 前缀；`t_group=journal` 使用 `tag:`、`loc:` 前缀。地点选项以路径保存，例如 `loc:城市/区域/地点`，手记实际值则是数组。删除选项和删除记录上的标签不是同一种写入。

## 不能按字段名直译的值

- Todo `schedule`：空值 / 过去日期表示当前可重新规划；Unix epoch 时间用作取消日期；毫秒时间戳 `4000000000000` 用作 No Plan。详见[规划](../dashboard/todo/planning-and-execution.md)。
- Todo `completed=true` 是归档；归档动作把 `created_at` 改为当前时间。该字段已不保证是最初录入时间。
- Watch `rate` 存 0–20，界面除以 2 显示 0–10 分。作品 `year` 与完成日期年份不是一回事；默认 2099 用作未填年份。
- Watch `checkpoints` 是 `[日期字符串, 累计进度]` 数组，`-1` 表示放弃；不是逐次增量。
- Echo 周号采用“当年第一个周一开始第 1 周”，不是直接套 ISO 周号；十年区间与年末边界见 [Echoes](../dashboard/echoes/README.md)。
- Entry `visibility` 默认 `PUBLIC`，Blog 使用 `Private / Public / Archived`；名称相近不代表共用发布规则。

## 数据迁移边界

正文与业务对象通常由前端两次请求分别创建；旧代码不提供整体事务。删除业务对象也没有统一级联清理正文 / 媒体。迁移应核对悬空引用、孤立正文、共享正文和软删除记录，不能只导出页面主表。

`d_tiptap.history` 在每日清理中只保留近期版本；`presigned_url` 会过期。迁移需要当前正文、仍存在的历史和对象存储文件，不能指望历史补回全部改动或永久复用签名 URL。

## 源码依据

- [全部模型](../../../third_party/dashboard/model)、[迁移脚本](../../../third_party/dashboard/migration/migrations.go)：表名、字段、演进后的值。
- [Todo hooks](../../../third_party/dashboard/client/src/hooks/use-todos.ts)、[Watch hooks](../../../third_party/dashboard/client/src/hooks/use-watches.ts)、[Entry hooks](../../../third_party/dashboard/client/src/hooks/use-entries.ts)：JSON 和跨表写入。
- [路由](../../../third_party/dashboard/router.go)：资源 API 使用 `Action` 分发；如 `/api/todo?Action=DoneTodo`。本文按用户行为归类，不按 HTTP endpoint 编排。
