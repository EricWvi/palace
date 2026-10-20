---
status: implemented
date: 2026-10-20
---

# 文章通过 API 连同媒体一次导入，作为 Owner 的 Moment 进入时间线；摘星按发布日期列出，阅读页沿用笔记页

摘星开放「文章」。文章是别人写的网页文章，由另一个项目抓取筛选后，用 Access Token 调用 `POST /api/articles` 导入：一次 multipart 请求带上标题、Markdown 正文、原始网址、可选的作者、发布日期，以及正文引用的全部图片和视频。服务端把文件存为 Palace 媒体，把正文中的占位引用改写成 `/api/media/<id>`，然后在一个事务里写入文章和它的媒体。同一 Owner 下原始网址唯一，重复导入返回已有文章。每篇文章是一个 `kind = 'article'` 的 Moment，发生时间是导入时间，在时间线上显示为 `[ 文章 ] 标题 · 作者`；摘星的文章列表则按发布日期排列。文章页的版式、目录和 Markdown 展示与笔记页相同，另有 `···` 菜单：可以编辑正文以外的字段，也可以连同媒体一起删除。导入接口的用法见[文章功能文档](../../../../docs/features/articles/README.md)。

当前为 `implemented`，核心测试用例见[摘星文章](../../../test-cases/web/stars/articles.md)。本决策基于[笔记来自挂载的 Markdown 文件夹](20261014-notes-from-mounted-folder.md)，开放其中预留的文章类型；媒体的读取与引用规则见[媒体读取决策](../../server/media/20261020-read-media-by-redirect-to-presigned-url.md)。涉及 `palace-db` 的新表与迁移、`palace-backend` 的文章接口与时间线、`palace-domain` 的字段校验、`apps/palace-web` 的摘星、文章页、时间线卡片与 `MarkdownContent`。文章不进入同步域；旧数据库中的内容、抓取项目本身、跨类型搜索都不在范围内。

## 问题与约束

- 抓取和筛选在另一个项目里完成，只有值得永久保存的文章才导入 Palace。那个项目里每篇文章是内嵌了图片、视频的 HTML 文件；导入 Palace 时传结构化数据和独立的文件，不传 HTML。
- 文章需要在时间线上出现（导航设计：“文章在时间线上以 `[ 文章 ]` 出现”），而 Moment 是 Owner 数据。导入方通过 Owner 级 Access Token 调用 API，天然带着 Owner。
- 文章的发布日期属于文章内容，Moment 时间回答的是“我哪天收下了它”，两者不是同一个时间。
- 原站会改版、下线，图片也随之失效。值得永久保存的文章，它的图片和视频必须由 Palace 自己保存。
- Palace 主要由 Eric 自用；文章页与笔记页共享版式，跳过原型设计。

## 继承与修改

| 前序约定 | 本决定 |
| --- | --- |
| 笔记决策 D3：挂载进摘星的只读内容（含将来的文章）对所有已登录用户可读，不属于 Owner | 修改：文章是 Owner 数据，带 `owner_id`，只在 Owner Scope 内可读写，见 D1 |
| 笔记决策 D2：Palace 对笔记只读，没有 `···` 菜单 | 不适用于文章：文章页有 `···` 菜单，可编辑正文以外的字段、可删除，见 D6 |
| 摘星根决策 D1：每种类型的列表用自己的路径，与详情页 `/<类型>/:id` 成对 | 继承；文章为 `/articles` 与 `/articles/<id>` |
| 摘星根决策 D2：类型列「笔记 / 文章 / 对话」，未开放类型不是链接 | 继承；文章改为可打开并显示条数 |
| 摘星根决策 D3：一行一条，标题 + 来源 + 点线 + `MM.DD`，按年分组；日期与时间线一致 | 修改：文章行的日期是发布日期，与时间线上的导入日期不同；来源是作者，见 D5 |
| 摘星根决策 D4：`/` 搜索、回车执行、`?q=`、标题高亮、“没有找到。” | 继承，不变；匹配标题、作者与正文，见 D5 |
| 摘星根决策 D5：分页接口 `{ items, next_cursor, total }`，keyset 游标，`total` 不受 `q` 影响 | 继承，不变 |
| 摘星根决策 D6：后退恢复列表滚动位置；从摘星进入的详情页不显示返回链接 | 继承，同样适用于文章页 |
| 笔记决策 D7：笔记页版式、首行一级标题去重、目录规则 | 继承，文章页相同；页首元信息与菜单不同，见 D6 |
| [Moment 根决策](../../server/moment/0-moment-supertype-and-typed-details.md) D1：`kind` 是封闭枚举，目前只有 `conversation` | 修改：增加 `article`，详情表 `article` 按 D2 的复合外键锁定类型，见 D3 |
| [Markdown 展示决策](../../server/conversation/20261010-shared-markdown-content-with-code-highlighting.md) D5：`load` 规则的 `resolve` 返回地址或 `null`，只渲染图片 | 修改：`resolve` 同时返回媒体种类，视频渲染为 `<video>`，见 D7 |
| [Access Token 决策](../../server/owner/20261018-owner-access-tokens-for-cli-and-services.md)：业务路由同时接受 Session 与 Access Token | 继承；文章导入、编辑、删除都是业务路由 |

## D1：文章是 Owner 数据

- 文章、文章的 Moment、文章引用的媒体都带 `owner_id`，导入时取认证上下文的 Owner；其他 Owner 读不到、改不了，响应与“不存在”相同（404）。
- 笔记决策 D3 曾预留“文章同样对所有已登录用户只读”。那时设想文章像笔记一样是部署级挂载内容；实际文章由 API 导入、要进时间线、要保存自己的媒体，这三者都要求一个 Owner。导入方本来就持有 Owner 级 token，绑定 Owner 不多一项配置。

## D2：字段与校验

| 字段 | 必填 | 规则 |
| --- | --- | --- |
| `title` | 是 | 去除首尾空白后非空，UTF-8 不超过 1024 字节（与 Path 标题相同）；原样保存 |
| `content` | 是 | Markdown 正文，非空，UTF-8 不超过 4 MiB；导入后不可修改 |
| `url` | 是 | 绝对 `http`/`https` URL，带主机名，不含首尾空白，不超过 2048 字节；原样保存，不做规范化 |
| `author` | 否 | 去除首尾空白后非空，不超过 256 字节；缺失、`null` 或纯空白都记为无作者 |
| `date` | 是 | `YYYY-MM-DD`，或带时区偏移的 RFC 3339 时间，见下文 |

- 只有日期的 `date` 按 `PALACE_TIMEZONE` 的当天 00:00 解释为时间点，与笔记的 `date` 相同。存储为 `published_at timestamptz` 与精度 `published_precision`（`day` / `instant`）；RFC 3339 时间截断到毫秒，满足毫秒精度约束。
- 接口返回 `published_at`（epoch 毫秒）和 `date`：精度为 `day` 时是 `PALACE_TIMEZONE` 下的 `YYYY-MM-DD`，为 `instant` 时是 `PALACE_TIMEZONE` 偏移下的 RFC 3339。编辑表单预填这个字符串，提交时按同一规则解析，因此只写了日期的文章编辑后仍只有日期。
- 记录精度而不是一律存日期：抓取项目有时能拿到精确到分钟的发布时间，有时只有日期。若一律存成 `date`，精确时间会丢失；若一律存成时间点，只写了日期的文章在编辑表单里会变成 `00:00:00+08:00`，看起来像录入了一个不存在的时刻。

## D3：存储、Moment 与时间线

- 新表 `article`：`id`（等于 Moment `id`）、`owner_id`、`kind`（恒为 `article`）、`title`、`content`、`url`、`author`、`published_at`、`published_precision`。`(owner_id, id, kind)` 外键引用 `moment`，与对话详情表相同。`UNIQUE (owner_id, url)` 支撑幂等导入（D4）。
- 新表 `article_media`：`(owner_id, article_id, media_id)`，`media_id` 唯一。一个媒体只属于一篇文章，文章删除时它的媒体一起删除（D6）。
- Moment 的 `occurred_at` 是导入时间，由数据库时钟写入（毫秒精度），此后不随编辑改变。时间线回答“那天我收下了什么”，文章写于哪天由 `published_at` 回答。
- 时间线接口的文章项：`id`、`kind = 'article'`、`occurred_at`、`title`、`author`、`site`（`url` 的主机名，去掉开头的 `www.`）。卡片显示为一行 `[ 文章 ] 标题 · 作者`，没有作者时显示 `site`；点击进入 `/articles/<id>`。不显示摘录：文章标题已经说明内容，开头几段往往是导语或站点模板。
- 文章正文加 `pg_trgm` GIN 索引，供 D5 的搜索使用。
- 文章不进入同步域，不分配 `serverVersion`。移动端同步文章时另行决定。

## D4：导入接口

`POST /api/articles`，`multipart/form-data`：

| part | 次数 | 内容 |
| --- | --- | --- |
| `article` | 1，必须是第一个 part | JSON：`{ title, content, url, author?, date }` |
| `file` | 0 或多次 | 一个媒体文件；`filename` 是正文中的引用名，`Content-Type` 是媒体类型 |

正文用 `media:<filename>` 作为图片或视频的地址引用文件：

```markdown
![封面](media:cover.jpg)

![演示](media:demo.mp4)
```

处理顺序与结果：

1. 解析 `article` 并按 D2 校验；失败返回 400，不读后续文件。
2. 在 Owner Scope 内按 `url` 查找。已存在时不再读取文件，直接返回 `200` 和已有文章（不比较、不更新内容）。
3. 逐个读取 `file`：每个文件按媒体根决策 D3 的上限与类型规则处理，生成媒体 ID 后立即写入对象存储，此时不写 `media` 行。
4. 校验引用：引用名只能由 `[A-Za-z0-9._-]` 组成、长度 1–128，同一请求内唯一（重复的文件名在写入第二个对象之前就被拒绝）；正文中的每个 `media:` 引用都必须有对应文件，每个文件都必须被正文引用。正文只在行内图片语法 `![](...)` 中识别 `media:`，代码块、行内代码与普通链接中的文字不算引用；引用式图片 `![alt][ref]` 的地址指向 `media:` 时直接拒绝，因为它的地址写在可被其他链接共用的定义里。失败返回 400。
5. 把每个 `media:<filename>` 改写为 `/api/media/<id>`，只替换地址本身，正文其余字节不变。
6. 在一个事务中写入 Moment、`article`、`media` 行与 `article_media`，返回 `201` 和文章。
7. 第 3 步之后任一步失败，尽力删除本次已写入的对象再返回错误；第 6 步因 `(owner_id, url)` 冲突失败（并发导入同一网址）时，删除本次对象并返回 `200` 和先提交的文章。

- 整个请求的大小上限由新环境变量 `PALACE_ARTICLE_MAX_BYTES` 决定，默认 256 MiB；单个文件仍受 `PALACE_MEDIA_MAX_BYTES` 约束。文件逐个缓冲、逐个写入，峰值内存约为一个文件的大小。
- 存储未配置时，带 `file` 的请求返回 `503 media_storage_unavailable`；不带文件的请求照常导入。
- 正文里其他地址（原站外链、`data:` URI）原样保存。展示时只加载 Palace 媒体，其余显示占位（D7）。

**为什么一次请求而不是先调 `POST /api/media` 再建文章**：分两步时，导入方在上传几张图之后失败或放弃，就会留下 Palace 有记录、却没有任何文章引用的媒体，而且没有任何东西会去回收它们。一次请求让 `media` 行与文章同一事务提交：要么文章和它的媒体一起出现，要么都不出现。对象存储无法加入事务，失败时最坏只留下没有 `media` 行的对象，与媒体根决策已有的孤儿风险相同。

**为什么由服务端改写引用**：媒体 ID 由服务端生成（媒体根决策 D1），导入方在发请求时还不知道 ID，只能用自己的名字引用文件。

**为什么按网址幂等而不是不去重**：抓取项目保证不重复导入同一篇，但网络超时后的重试无法区分“没导入”和“导入了但没收到响应”。按网址幂等让重试总是安全的，不需要额外的 idempotency key。命中已有文章时不更新内容：导入是“收下”，修正字段走编辑（D6），否则一次误触的重新导入会悄悄覆盖手动改过的标题。

## D5：摘星文章列表

`GET /api/articles`，参数与响应形状同摘星根决策 D5；`items` 每项为 `{ id, title, author, site, published_at, date }`。

- 按 `published_at DESC, id DESC` 排序，keyset 游标编码 `(published_at, id)`。
- 一行是 `标题  来源 ······ MM.DD`：来源显示作者，没有作者时显示 `site`；日期是发布日期，按浏览器本地时区显示，年份分组同样按发布日期。
- 搜索按子串、不区分大小写匹配标题、作者或正文，`%`、`_`、`\` 按字面量转义，与对话搜索相同。正文中改写后的 `/api/media/<id>` 也会被搜到，与对话搜索“链接地址等不可见文字也能被搜到”的取舍一致。

**为什么按发布日期而不是 Moment 时间**：摘星的目录回答“是哪一篇、什么时候写的”；按导入时间排列时，同一天批量导入的几十篇文章会挤在一起，目录上的日期也说明不了文章本身。代价是同一篇文章在摘星和时刻里显示不同的日期，这两个日期回答的是不同问题。

## D6：文章页、编辑与删除

`GET /api/articles/{id}` 返回 `{ id, title, content, url, author, site, published_at, date, occurred_at, media }`，`media` 是 `[{ id, content_type }]`，供前端区分图片与视频。

- 页首与笔记页相同：标题，下面一行元信息 `作者 · YYYY.MM.DD · 原文 ↗`。没有作者时省略作者；日期是发布日期；“原文 ↗”是指向 `url` 的链接，在新窗口打开并带 `rel="noreferrer"`，箭头沿用对话阅读页“继续对话 ↗”表示外站的写法。正文首行一级标题去重、目录、Markdown 展示与笔记页共用同一个阅读组件。不存在的文章显示“找不到这篇文章。”。
- 从时间线进入时地址带 `?date=`，页首上方显示回到那一天的链接，删除后也回到那一天，与对话阅读页相同；从摘星进入的文章页不显示返回链接，删除后回到 `/articles`。评审稿没有写时间线入口的返回方式，实现时按对话阅读页补上。
- `···` 菜单有“编辑文章”和“删除文章”，菜单与弹窗沿用对话阅读页。
- **编辑**：`PUT /api/articles/{id}`，请求体 `{ title, url, author, date }`，按 D2 校验后原子替换这四个字段，返回更新后的文章。正文不可编辑。新 `url` 已被本 Owner 的另一篇文章使用时返回 `409`，不做任何修改。编辑不改变 Moment 时间。多端同时编辑时后提交者生效，不做字段合并。
- **删除**：`DELETE /api/articles/{id}`。先按媒体根决策 D5 的顺序删除文章所有媒体的对象，再在一个事务中删除 `article_media`、`media` 行、`article` 与 Moment，返回 204。任一步失败返回 5xx，文章仍在，重试会重新走完全部步骤；中途失败时文章可能暂时缺少部分图片，重试后收敛。
- 改了 `url` 之后，用原网址重新导入会得到一篇新文章：幂等以当前网址为准。

**为什么正文不可编辑**：正文是别人的文章，保留原貌是收藏的意义所在；正文里还带着媒体引用，允许编辑就要处理媒体的增删与引用变化。标题、网址、作者、日期则常常抽取得不准，需要事后修正。

## D7：`MarkdownContent` 的媒体规则支持视频

`load` 规则的 `resolve` 从“返回地址或 `null`”改为返回 `{ kind: "image" | "video", src } | null`：

- 文章页的 `resolve` 只接受 `/api/media/<id>`，并且 `<id>` 必须出现在本文 `media` 中：`content_type` 为 `video/*` 时返回 `video`，否则返回 `image`；其他地址返回 `null`，显示“[图片：alt]”占位，不向原站发请求。
- `video` 渲染为 `<video controls preload="metadata">`，`aria-label` 取 alt；`image` 沿用现有的懒加载与 `referrerpolicy="no-referrer"`。
- 笔记的 `resolve` 对所有地址返回 `image`，行为不变；对话与卡片笔记仍用 `placeholder`。

**为什么不让前端按扩展名猜视频**：媒体地址 `/api/media/<id>` 没有扩展名，类型只在服务端的 `media.content_type` 里；由文章接口带出 `media` 列表，前端不需要为每个媒体再请求一次。

## 不变量

1. 文章及其媒体只在所属 Owner 的认证上下文中可见、可改；其他 Owner 得到与不存在相同的 404。
2. 同一 Owner 下 `url` 唯一；同一网址的重复导入或并发导入最终只有一篇文章，后到的请求得到这篇文章。
3. 导入成功时，正文中每个 Palace 媒体引用都指向本文的一个媒体，本文的每个媒体都被正文引用；导入失败时不留下 `media` 行或文章。
4. 导入后 `content` 不再改变；编辑只改变 `title`、`url`、`author`、`published_at` 与其精度。
5. 文章 Moment 的 `occurred_at` 是导入时间，编辑不改变它；摘星列表的顺序与日期只由 `published_at` 决定。
6. 删除接口返回成功时，文章、Moment、文章的媒体行和对象都已删除；删除失败时文章仍可打开。
7. 文章页只加载本文的 Palace 媒体，不向任何第三方地址发起图片或视频请求。

## 为什么不是这些替代方案

| 替代方案 | 优势 | 取舍 |
| --- | --- | --- |
| 沿用笔记决策 D3，文章是部署级只读内容 | 与笔记一致，无需 Owner | 无法成为 Moment 进入时间线，媒体也没有 Owner 可归属（媒体根决策要求 `owner_id`） |
| 先 `POST /api/media` 再 `POST /api/articles` | 复用现有上传接口；单个请求小 | 中途失败留下无人引用、无人回收的媒体；文章与媒体无法同事务出现 |
| 正文保留原站图片外链 | 不需要媒体存储 | 原站失效后图片丢失，违背“永久保存”；展示时向第三方发请求 |
| 图片以 `data:` URI 内嵌在正文 | 一个字段装下全部内容 | 正文膨胀到数十 MiB，列表、搜索索引和每次读取都要搬运它；视频不可行 |
| 不去重，每次导入都新建 | 实现最简单 | 超时重试产生重复文章，Palace 无从判断哪篇是多余的 |
| 同网址重新导入时覆盖内容 | 可以借重新导入修正 | 覆盖手动编辑过的字段；正文改变还要回收旧媒体，导入变成不可预期的更新 |
| Moment 时间用发布日期 | 摘星与时刻日期一致 | 几年前的文章会落到几年前的时间线上，与“那天收下了什么”无关，还会混进那天真实发生的事 |
| 摘星按导入时间排序 | 与对话列表“日期 = Moment 时间”的规则一致 | 批量导入的文章挤在同一天，目录日期说明不了文章本身 |
| 发布日期只存 `date` | 简单，无精度字段 | 丢失抓取到的精确时间 |
| 前端按 URL 扩展名判断视频 | 不需要 `media` 列表 | 媒体地址没有扩展名 |

## 风险与为什么不能直接改写

- **正文中的媒体地址**：导入后正文里写死了 `/api/media/<id>`，这条路径与媒体 ID 都不能改写，见媒体读取决策的风险一节。
- **孤儿对象**：导入在写对象之后、事务提交之前崩溃，会留下没有 `media` 行的对象，与媒体根决策的孤儿风险相同，靠以后的对账清理。
- **请求体大小**：带视频的文章可能接近 `PALACE_ARTICLE_MAX_BYTES`。超过上限时整篇被拒绝（413），由抓取项目决定是否降低视频质量；Palace 不做部分导入。

## 本决策未解决的问题

- **抓取项目的导入脚本**：在那个项目里实现，只依赖 D4 的接口。
- **文章的 tag 与分类**：与笔记的 tag 一起决定。
- **跨类型搜索**：仍只搜当前类型。
- **移动端同步文章**：文章暂不进入同步域。

## 落地与验收

- 迁移 `0017_articles.sql`：`moment.kind` 增加 `article`，新建 `article`、`article_media` 与正文 trigram 索引；提交时检查 Moment 详情的触发器覆盖文章；正文由触发器禁止修改。
- `palace-domain`：`DocumentDate` 解析与写回文档日期，笔记 cache 改为共用它；`ArticleMetadata` 校验 D2 的字段；`ArticleBody` 识别并改写 `media:` 引用。
- `palace-db`：文章的读写、按 `url` 查找、并发导入同一网址时返回先提交的文章、列表与搜索、时间线文章卡片。对话列表的游标改名为通用的 `ListCursor`，文章列表复用它。
- `palace-media`：`ArticleStaging` 按 D4 的顺序暂存对象、提交或回收；`delete_article` 按 D6 删除。
- `palace-backend`：`/api/articles` 与 `/api/articles/{id}` 的接口，导入单独挂载并使用 `PALACE_ARTICLE_MAX_BYTES` 作为请求体上限；`Server` 带上部署时区，用于解析和写回发布日期。
- `apps/palace-web`：摘星类型列开放文章，`/articles` 与 `/articles/<id>`，笔记页与文章页共用阅读组件，文章菜单与编辑、删除弹窗，时间线文章卡片，`MarkdownContent` 的视频规则。

证据见核心测试用例：领域单元测试覆盖字段、日期与引用改写；真实 PostgreSQL 覆盖类型锁定、Owner 隔离、网址唯一、编辑与列表；真实 PostgreSQL 加真实 MinIO 覆盖导入的原子性、barrier 驱动的并发导入和删除的失败重试；真实 Router 覆盖接口形状、导入与删除的完整流程及 OpenAPI 合同；组件测试覆盖摘星、文章页、编辑删除与时间线卡片。

笔记、对话与时间线已有的行为保持不变。
