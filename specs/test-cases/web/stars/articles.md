# 摘星文章核心测试用例

当前决策：[文章通过 API 连同媒体一次导入](../../../decisions/web/stars/20261020-articles-imported-with-media.md)。列表样式、搜索交互与后退恢复沿用[摘星入口页](contents-page.md)的用例，笔记页共享的版式与目录沿用[摘星笔记](notes.md)的用例，媒体读取见[媒体读取](../../server/media/media-reading.md)；本文件只记录文章引入的新承诺。数据库约束、事务与并发只能以真实 PostgreSQL 为直接证据，对象写入与删除只能以真实 MinIO 为直接证据。

## Article import must commit the article and its media together or not at all

风险：导入中途失败后留下没有文章引用的 `media` 行；正文里留着指向不存在媒体的引用，或上传的文件没有被正文引用；改写引用时误改了代码块里的文字或正文其他字节。前置：真实 Router、真实 PostgreSQL 与真实 MinIO；Access Token 属于 Owner A。触发：导入一篇正文引用两张图片和一个视频、并附带这三个文件的文章；分别导入引用了未上传文件、上传了未引用文件、引用名含非法字符、`article` 不是第一个 part、字段不合格（空标题、非 http URL、无法解析的 `date`）的请求；在对象写入之后、事务提交之前注入数据库失败；正文代码块中包含 `![](media:x.png)` 字面文字。

必须成立：成功时返回 201，正文中三个 `media:` 引用被改写为 `/api/media/<id>`，其余字节与原文相同，代码块中的字面文字不变；`article_media` 恰好关联这三个媒体，`media.content_type` 等于各 part 的类型；文章是 `kind = 'article'` 的 Moment，`occurred_at` 是导入时的数据库时间。每种不合格请求返回 400，失败或注入错误后数据库中没有新的 `media`、`article` 或 Moment 行，本次写入的对象已被尽力删除。带文件但存储未配置时返回 503，不带文件的导入照常成功。禁止部分导入，禁止靠 `sleep` 制造中途失败。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 成功导入改写全部引用、只改地址，媒体与文章同事务出现（真实 PostgreSQL 与真实 MinIO） | Missing | — |
| 引用与文件不一一对应、引用名非法、字段不合格时返回 400 且不留行 | Missing | — |
| 对象写入后注入数据库失败：不留行，本次对象被删除 | Missing | — |
| 代码块与行内代码中的 `media:` 不被识别为引用（领域单元测试） | Missing | — |
| 只有日期的 `date` 按 `PALACE_TIMEZONE` 换算并记录 `day` 精度；RFC 3339 截断到毫秒并记录 `instant` 精度 | Missing | — |

决策依据：D2、D3、D4，不变量 3。

## Importing the same url must converge on one article

风险：网络超时后的重试产生重复文章；并发导入同一网址时两篇都写入，或后到的请求失败并留下孤儿对象；重新导入悄悄覆盖了手动编辑过的字段。前置：真实 PostgreSQL 与真实 MinIO；Owner A 与 Owner B。触发：A 导入一篇文章后，以相同 `url`、不同标题与文件再次导入；两个请求以 barrier 卡在对象写入之后、事务提交之前，然后同时提交同一 `url`；B 导入与 A 相同的 `url`。

必须成立：重复导入返回 200 和原文章，标题、正文、媒体都不变，且不写入任何对象；并发导入最终只有一篇文章，后提交者返回 200 和先提交的文章，它写入的对象已被删除；B 的导入独立成功，得到自己的文章。禁止以 `sleep` 制造交错。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 重复导入返回原文章、不更新、不写对象（真实 PostgreSQL 与真实 MinIO） | Missing | — |
| 并发导入同一网址只得一篇，后到者删除自己的对象（barrier 驱动） | Missing | — |
| 网址唯一只在 Owner 内生效 | Missing | — |

决策依据：D4，不变量 2。

## Articles must stay within the owner scope

风险：文章接口没有按 Owner 过滤，其他 Owner 能列出、读取、编辑或删除别人的文章，或通过 409 等响应探知别人的网址。前置：真实 Router 与真实 PostgreSQL；Owner A 有文章，Owner B 有另一篇。触发：B 请求 A 文章的 `GET`、`PUT`、`DELETE`；B 列出与搜索文章；B 把自己文章的 `url` 改成 A 文章的 `url`；未登录请求全部文章接口。

必须成立：B 对 A 文章的读写都得到与不存在相同的 404，A 的文章不变；B 的列表、搜索和 `total` 只包含自己的文章；B 改成 A 的网址成功（网址唯一只在 Owner 内）；未登录请求被拒绝。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 跨 Owner 的读、改、删都是 404 且不改变数据（真实 PostgreSQL 与真实 Router） | Missing | — |
| 列表、搜索、`total` 只含本 Owner 的文章 | Missing | — |

决策依据：D1，不变量 1。

## Article list must order by published date while the timeline uses import time

风险：摘星列表按导入时间排列，批量导入的文章挤在同一天；或时间线把几年前发表的文章放到当年；编辑发布日期改变了 Moment 时间。前置：真实 PostgreSQL；多于一页（50 篇）的文章，导入顺序与发布日期相反，若干篇发布时间相同，部分有作者、部分没有；`PALACE_TIMEZONE` 与 UTC 不同。触发：逐页请求 `GET /api/articles` 直到 `next_cursor` 为 `null`；带 `q` 搜索只出现在作者、只出现在正文、含 `%` 的关键词；请求导入当天的时间线；编辑一篇文章的 `date`。

必须成立：拼接后每篇恰好出现一次，顺序为 `published_at DESC, id DESC`，与导入顺序无关；没有作者的行 `site` 为去掉 `www.` 的主机名；搜索匹配标题、作者或正文，`%` 按字面量匹配；导入当天的时间线包含全部文章项，字段为 `id`、`kind`、`occurred_at`、`title`、`author`、`site`；编辑 `date` 后列表顺序改变，而 Moment 的 `occurred_at` 与时间线位置不变。无法解析的游标返回 400，`total` 不受 `q` 影响。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 列表按发布时间 keyset 分页，每篇恰好一次（真实 PostgreSQL） | Missing | — |
| 搜索覆盖标题、作者与正文并转义通配符 | Missing | — |
| 时间线按导入时间出现，卡片字段完整；编辑不移动 Moment | Missing | — |

决策依据：D3、D5，不变量 5。

## Editing an article must replace only its metadata

风险：编辑改动了正文或媒体；把网址改成本 Owner 另一篇文章的网址后两篇文章网址相同；只写了日期的文章编辑后变成带时刻的时间点。前置：真实 PostgreSQL；Owner A 有两篇文章，其中一篇的 `date` 只有日期。触发：`PUT` 修改标题、作者、日期；`PUT` 清空作者；`PUT` 把网址改成另一篇的网址；`GET` 只有日期的文章后原样提交 `date`；Web 端从 `···` 菜单打开“编辑文章”，修改后保存，以及保存失败后重试。

必须成立：成功时四个字段被原子替换，`content`、`media`、`occurred_at` 不变；纯空白作者记为无作者；网址冲突返回 409，两篇文章都不变；只有日期的文章往返后精度仍为 `day`；Web 端保存后文章页、摘星列表与时间线卡片显示新值，失败时弹窗保留输入。禁止任何接口修改 `content`。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 编辑只替换四个字段，正文、媒体、Moment 时间不变（真实 PostgreSQL） | Missing | — |
| 网址冲突 409 且不做任何修改 | Missing | — |
| 日期精度往返不变 | Missing | — |
| 编辑弹窗保存后各处投影一致，失败保留输入（组件测试） | Missing | — |

决策依据：D2、D6，不变量 4。

## Deleting an article must remove its media objects before reporting success

风险：文章删除后它的图片和视频仍留在对象存储里；对象删除失败时接口仍报告成功；中途失败后文章已经消失、无法重试。前置：真实 PostgreSQL 与真实 MinIO；一篇带两个媒体的文章。触发：在删除第二个对象时注入失败后再重试；正常删除；Web 端从 `···` 菜单删除。

必须成立：注入失败时返回 5xx，文章、Moment、`media` 行仍在，文章仍可打开；重试后返回 204，文章、Moment、`article_media`、`media` 行与两个对象全部删除，读取这两个媒体得到 404；Web 端删除后跳转到 `/articles`，列表不再出现这篇文章。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 先删对象、后在事务中删行；中途失败可重试收敛（真实 PostgreSQL 与真实 MinIO，注入失败） | Missing | — |
| 删除成功后媒体读取返回 404 | Missing | — |
| Web 端删除后的落点与列表刷新（组件测试） | Missing | — |

决策依据：D6，不变量 6。

## Article page must load only its own Palace media

风险：文章页向原站发起图片请求，泄露阅读行为或在原站失效后显示破图；视频被当作图片渲染；其他文章的媒体被当作本文媒体加载。前置：一篇文章正文包含本文的图片与视频引用、原站外链图片、`data:` URI 图片，以及另一篇文章的 `/api/media/<id>`。触发：打开 `/articles/<id>`；打开不存在的文章；在摘星类型列点击「文章」。

必须成立：本文图片渲染为 `<img loading="lazy" referrerpolicy="no-referrer">`，本文视频渲染为 `<video controls preload="metadata">`；外链、`data:` URI 与另一篇文章的媒体都显示“[图片：alt]”占位，不发起请求；页首显示标题与 `作者 · YYYY.MM.DD · 原文`，没有作者时省略作者，“原文”在新窗口打开并带 `rel="noreferrer"`；不存在时显示“找不到这篇文章。”；类型列中「文章」是链接并显示条数。笔记的图片与对话的占位规则不变。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 只加载本文媒体，按类型渲染图片或视频，其余为占位（组件测试） | Missing | — |
| 页首元信息、原文链接与找不到的提示（组件测试） | Missing | — |
| 类型列开放文章并显示条数 | Missing | — |
| 笔记与对话的图片规则不受 `resolve` 改动影响 | Missing | — |

决策依据：D6、D7，不变量 7。
