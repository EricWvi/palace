# 摘星笔记核心测试用例

当前决策：[笔记来自挂载的 Markdown 文件夹](../../../decisions/web/stars/20261014-notes-from-mounted-folder.md)。列表样式、搜索交互与后退恢复沿用[摘星入口页](contents-page.md)的用例；本文件只记录笔记引入的新承诺。决策尚未实现，所有证据当前为 `Missing`。

## Stars navigation must open the note list and keep articles inert

风险：摘星仍落到对话列表，或笔记开放后导航当前项错乱；尚未开放的文章被误开放。前置：已登录，笔记索引中有若干笔记，另有对话。触发：点击顶部导航的「摘星」；分别打开 `/notes`、`/notes/<id>`、`/conversations`、`/conversations/:id`，读取无障碍树；用键盘 Tab 遍历类型列。

必须成立：「摘星」是指向 `/notes` 的链接；四种地址上导航中只有摘星带 `aria-current="page"`。类型列中「笔记」与「对话」都是链接并显示条数，当前类型带 `aria-current="page"`；「文章」不是链接、不进入 Tab 顺序、不显示条数。没有 `date` 的对话阅读页删除整个对话后仍落到 `/conversations`。禁止摘星指向 `/conversations`，禁止用 `?kind=` 选择类型。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 摘星指向 `/notes`，笔记与对话的列表页、详情页上只有摘星是当前项 | Missing | 无 |
| 类型列中笔记与对话可打开并显示条数，文章保持不可达 | Missing | 无 |
| 对话删除后的落点仍是 `/conversations` | Partial | `pages/conversation.test.tsx::after deleting %s, replaces the page with %s`（现有行为，摘星改指向后需复核） |

决策依据：D1，不变量 1。

## Notes must be read-only and readable by every signed-in user

风险：笔记接口按 Owner 过滤导致其他登录用户看不到；或 Palace 暴露了改写笔记文件的途径，与文件夹形成两份真相；未登录请求读到笔记。前置：真实 Router 与认证中间件；笔记文件夹以只读方式挂载（测试中为只读权限的临时目录），索引中有笔记；两个不同的 Owner。触发：两个 Owner 分别请求 `GET /api/notes` 与 `GET /api/notes/<id>`；未登录请求同样两个接口；打开笔记页查找任何操作入口。

必须成立：两个 Owner 得到完全相同的响应；未登录请求被拒绝；只读挂载下列表、搜索与笔记页全部正常；笔记页没有 `···` 菜单或任何编辑、删除、新建入口，`n` 不触发新建。禁止笔记接口读取或依赖 `owner_id`，禁止存在任何写笔记文件的路由。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 不同 Owner 的笔记响应相同，未登录被拒绝（真实 Router） | Missing | 无 |
| 只读挂载下所有笔记功能正常 | Missing | 无 |
| 笔记页没有任何改变笔记的入口 | Missing | 无 |

决策依据：D2、D3，不变量 2、3。

## Note list must page every indexed note exactly once by its date

风险：笔记按文件修改时间或文件名排序，目录日期随编辑漂移；分页途中 cache 被替换导致条目重复或遗漏；只有日期的 `date` 换算到错误的一天。前置：索引中多于一页（50 条）的笔记，若干篇 `date` 相同，部分只有日期、部分是带偏移的 RFC 3339 时间；文件 mtime 顺序与 `date` 相反；`PALACE_TIMEZONE` 与 UTC 不同。触发：不带 `q` 逐页请求 `GET /api/notes` 直到 `next_cursor` 为 `null`；取完第一页后替换 cache，加入一篇新笔记，再继续取后续页；传入无法解析的游标。

必须成立：拼接后索引中每篇笔记恰好出现一次，顺序为 `occurred_at DESC, id DESC`，与 mtime 无关；只有日期的 `date` 等于 `PALACE_TIMEZONE` 当天 00:00 的 epoch 毫秒；中途替换 cache 不使已有条目重复或丢失；`total` 等于当前索引的笔记数且不受 `q` 影响；无法解析的游标返回 400。禁止使用偏移量分页，禁止列表日期来自文件系统时间。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 逐页取完后每篇恰好一次，按 `date` 与 ID 排序，与 mtime 无关 | Missing | 无 |
| 只有日期的 `date` 按 `PALACE_TIMEZONE` 换算 | Missing | 无 |
| 翻页途中替换 cache 不重复、不遗漏；`total` 不受 `q` 影响；坏游标返回 400 | Missing | 无 |

决策依据：D4、D6，不变量 4、5。

## Cache script must enumerate the files search sees and reject invalid frontmatter

风险：脚本与 rg 对 ignore 规则理解不同，列表出现搜不到的文件，或被忽略的草稿出现在列表里；frontmatter 不合格的笔记被写进 cache，Palace 展示空标题或错误日期；写到一半的 cache 被服务端读到。前置：真实 ripgrep；临时笔记文件夹不在 git 仓库内，含 `.gitignore` 排除的目录、`.ignore` 排除的文件、隐藏目录、非 `.md` 文件；另准备缺 `title`、`date` 格式错误的笔记各一篇。触发：对合格文件夹运行脚本；再加入不合格笔记运行脚本。

必须成立：合格时 cache 恰好包含 `rg --files --no-require-git -g '*.md'` 列出的文件，ID 为去掉 `.md` 的相对路径且分隔符为 `/`，每项的 `title`、`date` 与 frontmatter 一致，不含 `tag` 等其他字段；cache 以 rename 原子替换。不合格时报告所有问题、以非零状态退出，原有 cache 保持不变。禁止 `.gitignore` 或 `.ignore` 排除的文件进入 cache，禁止 cache 自身被枚举。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 枚举结果与 rg 一致，不在 git 仓库内时仍应用 `.gitignore`（真实 ripgrep） | Missing | 无 |
| 不合格 frontmatter 全部报告、非零退出、不改动原 cache | Missing | 无 |
| cache 原子替换，只收集 `title` 与 `date` | Missing | 无 |

决策依据：D4、D5，不变量 7。

## Cache reload must swap the whole index and keep it when the cache is broken

风险：服务端监听不到原子替换后的 cache，笔记永远不更新；读到损坏或半写的 cache 时清空列表或只替换了一部分；启动时没有 cache 导致服务起不来。前置：真实文件系统上的临时笔记文件夹，服务端以它为 `PALACE_NOTES_DIR`；文件变化事件与轮询兜底可以分别注入或关闭。触发：以 rename 替换 cache；写入损坏的 JSON；写入未知 `version`；删除 cache；以没有 cache 的文件夹启动；关闭文件事件、只靠轮询时替换 cache。

必须成立：rename 替换后索引更新为新 cache 的完整内容；只靠轮询时也会在下一次轮询后更新；损坏、未知版本或缺失时保留上一份索引并记录错误；没有 cache 时以空索引启动，服务照常响应；未配置 `PALACE_NOTES_DIR` 时笔记列表为空、`total` 为 0。任何时刻请求读到的都是某一份完整索引。禁止因 cache 错误清空已有索引或拒绝启动，禁止靠 `sleep` 等待事件，以注入的事件或轮询触发驱动。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| rename 替换被检测到并整体替换索引（真实文件系统） | Missing | 无 |
| 轮询兜底在没有文件事件时生效 | Missing | 无 |
| 损坏、未知版本、缺失时保留上一份索引；无 cache 或未配置时以空索引启动 | Missing | 无 |

决策依据：D5，不变量 9。

## Note search must match literally and stay within the index

风险：关键词被当作正则或命令行选项；不在 git 仓库内时搜到被 `.gitignore` 排除的草稿；结果出现已删除或尚未进入 cache 的文件；rg 失败时返回部分结果。前置：真实 ripgrep；临时笔记文件夹不在 git 仓库内，含被 `.gitignore` 排除且包含关键词的文件、尚未进入 cache 的新文件、cache 中有但已删除的文件；笔记正文含 `.*`、`(`、以 `-` 开头的文字和大小写不同的英文。触发：分别以标题中的词、只在正文中的词、只在 frontmatter `tag` 中的词、`.*`、`(`、`-x`、不同大小写的词请求 `GET /api/notes?q=`；让 rg 超时或不可执行。

必须成立：每一行的标题或文件内容包含关键词（不区分大小写的字面量）；`.*`、`(`、`-x` 只匹配字面量且请求不报错；`tag` 中的词能搜到；被 `.gitignore` 排除的文件、不在索引中的新文件与已删除的文件都不出现；结果按列表顺序分页；rg 超时或不可执行时返回 5xx 而不是部分结果。禁止经由 shell 调用 rg，禁止结果超出当前索引。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 标题与正文按不区分大小写的字面量匹配，特殊字符与 `-` 开头的关键词安全（真实 ripgrep） | Missing | 无 |
| 不在 git 仓库内时仍应用 `.gitignore`，结果是当前索引的子集（真实 ripgrep） | Missing | 无 |
| rg 超时或不可执行时返回 5xx | Missing | 无 |

决策依据：D6，不变量 6、7。正文匹配的义务只能以真实 ripgrep 为直接证据。

## Note page must read only files listed in the index

风险：请求中的 ID 被直接拼成路径，借 `..`、绝对路径或编码后的分隔符读到笔记文件夹之外的文件；返回的正文仍带 frontmatter；文件在索引中但已删除时服务端报 500。前置：真实文件系统；笔记文件夹外有一个可读的秘密文件；索引中有一篇 ID 含 `/` 与中文的笔记、一篇已被删除的笔记；笔记文件夹内有一个不在索引中的 `.md` 文件。触发：请求 `GET /api/notes/<id>`，`<id>` 分别为合法 ID、`../secret`、`%2e%2e%2fsecret`、绝对路径、不在索引中的文件 ID、已删除文件的 ID；打开对应的笔记页。

必须成立：合法 ID 返回 `{ id, title, occurred_at, content }`，`content` 是去掉 frontmatter 的正文，并按请求时文件的最新内容返回；标题与日期来自索引。其余 ID 一律返回 404，已删除的文件同时记录日志。笔记页以共享 Markdown 组件渲染正文，显示标题与 `YYYY.MM.DD`，从摘星进入时没有返回链接。禁止任何请求读到笔记文件夹之外的文件或不在索引中的文件。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 路径穿越、绝对路径、不在索引中的 ID 全部 404，读不到秘密文件（真实文件系统与真实 Router） | Missing | 无 |
| 正文去掉 frontmatter 且读取最新文件内容，已删除文件返回 404 | Missing | 无 |
| 含 `/` 与中文的 ID 在前端路由与接口间往返一致，笔记页版式与无返回链接 | Missing | 无 |

决策依据：D4、D7，不变量 8。
