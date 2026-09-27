# Blog：发布版额外存在的博客管理页

[返回总览](../../README.md)。本页没有对应用户截图，但 `v2.17.3` 桌面导航明确包含 Blog；为避免 Palace 融合旧项目时遗漏，单独记录。移动底部导航没有 Blog 页签。

## 数据与页面

主表 `d_blog` 保存 `title`、`visibility`、`draft` 和 `payload.whats / hows`。正文使用 `d_tiptap`，媒体使用 `d_media`，标签与 Bookmark 共用 `d_tag.t_group=dashboard`。

列表展示创建时间、标题、What、How、更新时间及操作菜单；工具栏支持标题和两类标签过滤、分页、新建。状态通过标题相关展示呈现，不等于 Journal 的收藏或 Todo 的归档。

## BL01：创建与阅读文章

Add 输入标题和标签，前端先创建含文章标题一级 heading 的正文，再创建 `visibility=Private` 的 `d_blog`。默认是未发布文章。列表标题打开只读正文，菜单 Edit 打开编辑器。

## BL02：修改元信息与内容

元信息表单修改标题和两类标签，正文独立编辑。元信息标题和 Tiptap 内第一段标题没有自动双向同步，迁移应检查二者差异。正文直接更新 `d_tiptap`，不能假定 `d_blog.updated_at` 代表每次正文变动。

## BL03：发布、撤回、归档与恢复

| 初始状态   | 操作            | 写入状态   |
| ---------- | --------------- | ---------- |
| `Private`  | Publish，确认   | `Public`   |
| `Public`   | Unpublish，确认 | `Private`  |
| 非归档     | Archive，确认   | `Archived` |
| `Archived` | Unarchive，确认 | `Private`  |

这些操作只更新文章元信息，没有发现本版本提供匿名公共博客阅读路由或独立发布流水线。主应用列出的是当前用户自己的文章，不能把 Public 直接解释为“已经能通过公开 URL 访问”。行菜单也没有永久删除流程。

## 迁移核对

保留文章、正文、标签、三个状态及可能不同步的标题。Palace 是否保留独立 Blog 类型、如何实现公开发布需要另行决定；本清单只确认旧版管理能力存在。

源码：[桌面导航](../../../../third_party/dashboard/client/src/components/tabbed-app.tsx)、[页面](../../../../third_party/dashboard/client/src/pages/Blog.tsx)、[hooks](../../../../third_party/dashboard/client/src/hooks/use-blogs.ts)、[行菜单](../../../../third_party/dashboard/client/src/components/react-table/data-table-row-actions.tsx)、[Blog handlers](../../../../third_party/dashboard/handler/blog)、[路由](../../../../third_party/dashboard/router.go)。
