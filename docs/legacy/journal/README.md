# Journal：日常手记与回顾

[返回总览](../README.md) · [截图](../screenshots/Journal.png)

Journal 是独立前端入口，与 Dashboard 共用后端、用户、编辑器和附件能力。主界面围绕按时间倒序排列的手记卡片，支持富文本和照片 / 视频、标签地点、书签、检索、日历与回顾。

## 页面与数据

| 页面区域              | 数据表                                | 功能                                       |
| --------------------- | ------------------------------------- | ------------------------------------------ |
| 时间流 / 单条卡片     | `d_entry`、`d_tiptap`                 | 元信息与完整正文分开存储；按日期分组       |
| 写作弹层              | `d_tiptap`，保存摘要时 `d_entry`      | 富文本正文、字数和搜索文本                 |
| 标签 / 地点编辑及筛选 | `d_entry.payload`、`d_tag`            | 用户实际值与可选词库                       |
| 收藏 / 随机回顾       | `d_entry.bookmark / review_count`     | 重点标记与回顾均衡                         |
| 统计 / 日历           | `d_entry`                             | 今年篇数、总字数、有手记的天数、年度热力图 |
| 媒体查看              | 正文媒体节点、`d_media`               | 缩略图、图片 / 视频查看器                  |
| 分享                  | `d_user`、`d_entry`、`d_tiptap`、附件 | 导出带作者与日期的 PNG                     |

`d_entry` 没有独立标题列，卡片直接展示正文。`draft` 是正文 ID，不能用它判断是否“发布”；新建手记在进入编辑器前已经创建数据库记录。

## 能力索引

- [写作与分享](writing-and-sharing.md)：N01 新建、N02 编辑、N03 标签地点、N04 收藏、N05 删除、N06 媒体浏览、N07 图片分享。
- [检索与回顾](browsing-and-recall.md)：N08 时间流、N09 搜索、N10 日期、N11 标签地点过滤、N12 收藏过滤、N13 历史今天、N14 随机回顾、N15 统计日历。
- [通用编辑](../shared/editor-and-history.md)：编辑器格式、保存、历史恢复。

## 迁移范围与边界

截图菜单的日期、标签、位置、书签、历史今天、随机回顾都有对应调用链，不只是静态菜单。地点由用户维护三级文本，没有地图、地理坐标或自动定位流程。

Entry 模型保留 `visibility=PUBLIC/PRIVATE`，默认 PUBLIC，但本页没有可见性切换或匿名分享阅读页。分享是图片导出。该版也没有修改手记发生日期的页面 / handler 字段；`created_at` 是时间流依据。

旧版单列阅读、浮动新增按钮和图片排列用于说明已有功能，不构成 Palace 必须照搬的布局。后续统一日期轴、内容卡片和详情入口应另外设计。

源码：[Journal 入口](../../../third_party/dashboard/client/src/JournalApp.tsx)、[页面](../../../third_party/dashboard/client/src/pages/Journal.tsx)、[Entry 模型](../../../third_party/dashboard/model/entry.go)、[hooks](../../../third_party/dashboard/client/src/hooks/use-entries.ts)。
