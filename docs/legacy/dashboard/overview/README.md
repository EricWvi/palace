# Dashboard 首页

[返回总览](../../README.md) · [截图](../../screenshots/Dashboard.png)

首页把今日执行、随手记录和常用入口放在一起。桌面展示资料、Today、Quick Note 三块；移动端顶部是头像、昵称与按时段变化的问候，下面纵向排列 Today 和 Quick Note。

## 数据依赖

| 区域               | 表 / 外部来源                                      | 能力                                     |
| ------------------ | -------------------------------------------------- | ---------------------------------------- |
| 用户资料           | `d_user`，上传头像时 `d_media`                     | 展示与修改昵称、头像、语言               |
| Today              | `d_todo`、`d_collection`                           | 按集合聚合今日安排、规划今天、执行与撤销 |
| Quick Note         | `d_quick_note`、`d_tiptap`，正文媒体使用 `d_media` | 创建、阅读、编辑、改名、置底、删除       |
| Miniflux / QQ Mail | `d_user` 保存连接配置；外部服务提供未读数          | 显示未读角标、进入外部服务               |
| Journal / Beaver   | 嵌入外部站点                                       | 侧边面板内打开，不导入其内容到首页数据库 |

## D01：查看与执行今天安排

Today 不是另一个任务集合。它查询全部未归档、日期为今天的 `d_todo`，按集合分组，组内依事项顺序展示；做完的条目仍在其中。

右上加号打开“我的一天”批量选择已有事项，不直接新建标题。条目可以 Done / Undone、取消日期、改名、设置链接、编辑草稿 / 看板。详细状态与用例统一见 [Todo 规划与执行](../todo/planning-and-execution.md)。

## D02：快速新建一条笔记

点击 Quick Note 加号，先创建 `d_tiptap`，再创建 `d_quick_note`，标题默认是当天 `YYYY-MM-DD`，正文 ID 写入 `draft`，排序值为现有最大值加 1，随后直接打开编辑器。同一天可以创建多条，不以日期去重。

笔记不是 Journal 手记：不创建 `d_entry`，不进入 Journal 统计 / 时间流，也不带地点、收藏或随机回顾计数。

## D03：阅读、编辑和整理速记

| 场景 | 交互                   | 数据影响                               |
| ---- | ---------------------- | -------------------------------------- |
| 阅读 | 点击标题，弹框展示正文 | 读取引用的 `d_tiptap`                  |
| 编辑 | 菜单进入草稿编辑器     | 更新正文，采用共享保存与历史规则       |
| 改名 | 菜单 Rename            | 只改 `d_quick_note.title`              |
| 置底 | 非末项菜单 Bottom      | 修改 `d_order`，按降序重新排列         |
| 删除 | 菜单 Delete，确认      | 软删除速记元信息，正文和附件不级联删除 |

## D04：资料与外部工具

桌面 Profile 头像菜单可编辑昵称、头像与 `zh-CN / en-US` 语言；头像支持上传和清除。首次进入主应用、用户名为空时显示资料初始化页。

Profile 同时提供 Journal、QQ Mail、Miniflux、Beaver 入口。Journal / Beaver 通过 iframe 侧边栏打开；QQ Mail 打开新标签页，Miniflux 提供嵌入阅读入口。未配置 RSS / 邮箱凭据时引导配置；后端取未读数，前端显示非零角标。凭据配置细节见[应用基础](../../shared/application.md)。

Profile 在移动首页没有完整渲染，不能把桌面资料菜单和四个外部工具都视为移动首页已有能力。

## 迁移核对

- 今日任务与 Todo 中的同一事项保持一致。
- 速记有自己的标题和顺序，不能只按正文创建时间恢复顺序。
- 接入入口和内容归属分别核对：嵌入 Beaver 不代表旧库保存了 Beaver 业务数据。

源码：[Dashboard](../../../../third_party/dashboard/client/src/pages/Dashboard.tsx)、[Quick Note](../../../../third_party/dashboard/client/src/components/quick-note.tsx)、[Profile](../../../../third_party/dashboard/client/src/components/profile.tsx)、[Quick Note handlers](../../../../third_party/dashboard/handler/tiptap)。
