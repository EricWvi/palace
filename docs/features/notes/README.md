# 笔记

笔记是 Palace 之外维护的 Markdown 文件夹，挂载进容器后出现在摘星中。Palace 只读：不新建、编辑或删除笔记，
所有已登录用户看到同一份笔记，不属于某个 Owner。决策见
`specs/decisions/web/stars/20261014-notes-from-mounted-folder.md`。

## 文件与 cache

每篇笔记是一个 `.md` 文件，frontmatter 必须有非空的 `title`，以及 `YYYY-MM-DD` 或带时区偏移的 RFC 3339
`date`；其他字段（如 `tag`）保留在文件里，暂不使用。笔记 ID 是文件相对文件夹根目录的路径去掉 `.md`，例如
`rust/advisory-lock`，改名或移动文件会改变 ID。

服务端不解析 frontmatter，只读文件夹根目录的 `.palace-cache.json`。在宿主机上运行
`node scripts/notes-cache.ts <笔记文件夹>` 生成它：脚本用 ripgrep 的规则枚举笔记（尊重 `.gitignore`、`.ignore`，
跳过隐藏文件，不要求是 git 仓库），任何一篇不合格都列出全部问题、以非零状态退出且不改动原 cache。
挂载与配置见[环境变量](../../server/环境变量.md#摘星笔记)。

## 摘星中的笔记

顶部导航的「摘星」进入 `/notes`。列表样式与对话列表相同（标题、点线、`MM.DD`，按年分组），没有来源一栏；
日期与顺序取 frontmatter 的 `date`，倒序，同一时间按 ID 倒序。左侧类型列中「笔记」「对话」都可打开并显示各自的条数，
「文章」尚未开放。

按 `/` 打开搜索，回车执行：标题在内存中匹配，正文由容器内的 `rg` 搜索整个文件，二者都按不区分大小写的字面子串匹配；
frontmatter 也是文件的一部分，`tag` 中的词同样能搜到。结果只包含 cache 中的笔记。

点击一行进入 `/notes/<id>`（每段分别编码）。页首是标题和 `YYYY.MM.DD`；正文第一行若是一级标题，就由它作页首标题（不再显示 frontmatter `title`），日期在它下面。正文用共享的 Markdown 组件渲染，图片按原地址加载；
页面没有任何操作，也没有返回链接。正文每次打开时从文件读取，标题和日期以 cache 为准。

阅读页右侧提供标题目录，沿用对话目录的样式与交互。页首标题作为 H1，正文按实际渲染的 H1–H6 顺序
列出；标题里的 Markdown 格式只保留文字，同名标题各有独立锚点，代码块中的示例不进入目录。
H1 横线最长，H2–H6 依次变短；展开后按层级缩进。点击跳到对应标题并固定当前标记，直到读者自行滚动。
悬停 350ms 后展开，快速经过或移出浏览器会取消；键盘进入立即展开。标题不足两条或窗口窄于 900px 时隐藏。

## 接口

| 接口 | 说明 |
| --- | --- |
| `GET /api/notes?q=&cursor=` | 每页 50 条 `{ id, title, occurred_at }`，`total` 为全部笔记数且不受 `q` 影响；搜索无法完成时返回 503 `search_unavailable` |
| `GET /api/notes/{id}` | `{ id, title, occurred_at, content }`，`content` 是去掉 frontmatter 的正文；`{id}` 是一个路径段，`/` 编码为 `%2F`；不在 cache 中返回 404 |
