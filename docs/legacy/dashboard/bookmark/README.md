# Bookmark：网址与速查笔记

[返回总览](../../README.md) · [截图](../../screenshots/Bookmark.png)

Bookmark 把外部资源与内部 cheatsheet 放进同一张检索表，通过领域、What、How 三个维度组织。这里的 Domain 是知识领域，不是 URL 的 host。

## 页面与数据

| 表 / 字段                         | 用户功能                                                           |
| --------------------------------- | ------------------------------------------------------------------ |
| `d_bookmark.title / url`          | 资源名称、打开网址；特殊输入 `cheatsheet` 创建内部速查笔记         |
| `d_bookmark.domain`               | 技术、知识、健康、个人发展、社会文化、生活、金融、艺术、自然、杂录 |
| `d_bookmark.payload.whats / hows` | 两组多选字符串标签                                                 |
| `d_bookmark.payload.draft`        | cheatsheet 的 `d_tiptap` 正文 ID                                   |
| `d_bookmark.click`                | 通过列表打开资源 / cheatsheet 的累计次数                           |
| `d_tag`                           | `t_group=dashboard` 的选项词库，名称为 `what:...` / `how:...`      |
| `d_media`                         | cheatsheet 内上传的图片 / 视频                                     |

桌面列为 Domain、Title、Link、What、How 和行菜单。列表按创建时间倒序取回，在前端筛选分页；桌面初始每页 10 条，移动端适配页大小和表格显示。

## B01：收藏一个外部资源

Add 打开表单，填写标题、链接、领域，选择或新建 What / How 标签。提交创建 `d_bookmark`，实际标签数组保存在 payload 中；随后前端把新词补入 `d_tag`。

标签含义由用户定义：例如 What 可以是 tool / resource / post，How 可以是 ai / picture / video，不是固定枚举。创建词库与保存书签是分开的请求；不能把词库当作记录标签的唯一来源。

## B02：建立和编辑 cheatsheet

在 URL 输入精确字符串 `cheatsheet`，前端先创建 `d_tiptap`，再把 ID 写入 `payload.draft`。列表 Link 显示 cheatsheet，点击在弹框中阅读正文；行菜单的编辑正文入口打开共享编辑器。

已有普通网址改为 cheatsheet 时，缺少 `draft` 才新建文档。反向改回普通 URL 时，代码保留已有 `payload.draft`，而列表优先根据 `draft` 是否存在决定显示 cheatsheet。因此此版本没有完整的“清除速查正文、转换回网址”流程。迁移须同时检查 URL 和 draft，不能只看 URL 文本分类。

## B03：从多维度找到资源

工具栏可按标题文本过滤，按领域、What、How 筛选，清除筛选及翻页。领域选多个时匹配其中一个；What / How 每一组使用 `every`，即所选标签必须全部出现；不同维度共同收窄结果。

例如选 What 的 tool 与 cheatsheet，要求同一资源同时有两个标签，不是任选其一。该版没有搜索 cheatsheet 正文的数据库全文检索。

## B04：打开资源与记录使用次数

普通资源通过标题 / Link 打开新标签页，同时调用 `ClickBookmark` 增加 `click`。cheatsheet 打开只读弹框也增加次数。正文编辑不是点击计数入口。

点击次数保存在表中，但当前主表格没有独立次数列，也没有按点击热度排列的入口。不能因有 `click` 字段就认为已提供热门书签功能。

## B05：更新与删除

行菜单编辑标题、URL、领域和两组标签，保存不覆盖 `click`；新标签补入共享词库。Delete 确认后软删除书签，不清理速查正文、标签选项或媒体。

`DeleteTag` API 存在，但本页没有完整独立标签管理界面；词库删除也不等于从所有 payload 中移除对应字符串。Blog 共用 dashboard 词库，Journal 使用另一个 group。

## 迁移核对

- 普通网址与速查笔记混排，标题、领域、两组标签、正文和点击次数均可恢复。
- `domain` 的领域值与网站域名分别处理。
- 标签必须保留组别和前缀语义，不能把 What、How、Journal 标签混为一个无类型字符串集合。
- Journal 的收藏不会出现在本页，两种 Bookmark 语义独立。

源码：[页面](../../../../third_party/dashboard/client/src/pages/Bookmark.tsx)、[hooks 与领域列表](../../../../third_party/dashboard/client/src/hooks/use-bookmarks.ts)、[新增表单](../../../../third_party/dashboard/client/src/components/react-table/data-table-toolbar.tsx)、[编辑菜单](../../../../third_party/dashboard/client/src/components/react-table/data-table-row-actions.tsx)、[列表展示与筛选](../../../../third_party/dashboard/client/src/components/react-table/data-table-columns.tsx)、[模型](../../../../third_party/dashboard/model/bookmark.go)。
