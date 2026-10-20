# 文章

文章是别人写的网页文章，在另一个项目里抓取、筛选后，用 Access Token 调用 API 导入 Palace。导入时连同正文用到的图片和视频一起保存，此后不依赖原站。每篇文章是导入当天时间线上的一个 Moment，在摘星中按发布日期列出，文章页的版式与笔记页相同。决策见[文章决策](../../../specs/decisions/web/stars/20261020-articles-imported-with-media.md)，媒体读取见[媒体读取决策](../../../specs/decisions/server/media/20261020-read-media-by-redirect-to-presigned-url.md)。

## 导入

`POST /api/articles`，`multipart/form-data`，带 `Authorization: Bearer palace_pat_…`。第一个 part 必须是 `article`，内容是 JSON；之后每个嵌入的文件一个 `file` part：

```bash
curl https://palace.example.com/api/articles \
  -H "Authorization: Bearer $PALACE_TOKEN" \
  -F 'article={"title":"标题","content":"![封面](media:cover.jpg)\n\n正文……\n\n![演示](media:demo.mp4)","url":"https://example.com/post","author":"作者","date":"2026-10-05"};type=application/json' \
  -F 'file=@cover.jpg;filename=cover.jpg;type=image/jpeg' \
  -F 'file=@demo.mp4;filename=demo.mp4;type=video/mp4'
```

| 字段 | 必填 | 规则 |
| --- | --- | --- |
| `title` | 是 | 去除首尾空白后非空，UTF-8 不超过 1024 字节，原样保存 |
| `content` | 是 | Markdown 正文，非空，不超过 4 MiB；导入后不能修改 |
| `url` | 是 | 原文的绝对 `http`/`https` 地址，不超过 2048 字节；按字节原样比较，不做规范化 |
| `author` | 否 | 不超过 256 字节；缺失、`null` 或纯空白都表示没有作者 |
| `date` | 是 | 发布日期：`YYYY-MM-DD`（按 `PALACE_TIMEZONE` 的当天 00:00），或带时区偏移的 RFC 3339 时间 |

正文用 `![alt](media:<文件名>)` 嵌入文件：

- 文件名就是 `file` part 的 `filename`，只能由 `A-Z a-z 0-9 . _ -` 组成，长度 1–128，同一次导入里不能重复。
- 只有行内图片语法 `![alt](media:…)` 算嵌入；代码块、行内代码和普通链接里的 `media:` 不算。不能用引用式图片 `![alt][ref]` 指向文件。
- 正文嵌入的每个名字都要有文件，每个文件都要被正文嵌入，否则返回 400，什么都不保存。
- 视频也写成图片语法，`file` part 的 `Content-Type` 为 `video/*` 时，文章页显示为视频播放器。
- 保存后，服务端把每个 `media:<文件名>` 改写为 `/api/media/<id>`，正文其余字节不变。
- 原站外链和 `data:` URI 原样保存，但文章页只显示“[图片：alt]”占位，不会去加载。

响应：

| 状态 | 含义 |
| --- | --- |
| 201 | 新导入，返回文章 |
| 200 | 这个 `url` 已经导入过，返回已有文章，不比较也不更新内容，也不读取本次的文件 |
| 400 | 字段或嵌入关系不合规则，响应的 `path` 指出是哪一项 |
| 413 | 单个文件超过 `PALACE_MEDIA_MAX_BYTES`，或整个请求超过 `PALACE_ARTICLE_MAX_BYTES` |
| 503 | 带文件但部署没有配置媒体存储，或对象存储暂时不可用；什么都不保存 |

因为同一 `url` 的重复导入总是返回已有文章，超时或断线后可以直接重试同一个请求。

## 阅读、编辑与删除

- `GET /api/articles?q=&cursor=`：按发布日期倒序分页，每页 50 篇。`q` 按子串匹配标题、作者或正文，不区分大小写。
- `GET /api/articles/{id}`：返回文章和 `media` 列表（每项含 `id` 与 `content_type`）。`date` 字段是编辑表单使用的写法：只写了日期的文章仍然只有日期。
- `PUT /api/articles/{id}`：请求体为 `{ title, url, author, date }`，一起替换这四项，正文不变。新 `url` 已被另一篇文章使用时返回 409。
- `DELETE /api/articles/{id}`：先删除文章的所有媒体对象，再删除文章、Moment 和媒体记录。中途失败返回 5xx，文章仍在，重试即可。
- 文章嵌入的媒体不能用 `DELETE /api/media/{id}` 单独删除，返回 409 `media_in_use`。

Web 端：

- 摘星的类型列「笔记 · 文章 · 对话」中，「文章」进入 `/articles`。每一行显示标题、作者（没有作者时显示网站域名）和发布日期 `MM.DD`。
- 文章页 `/articles/<id>` 的页首是标题，下面一行是作者、发布日期和“原文 ↗”。正文第一行的一级标题会代替页首标题。目录规则与笔记相同。
- `···` 菜单里有“编辑文章”和“删除文章”。
- 时间线在导入当天显示 `[ 文章 ] 标题 · 作者`，点击进入文章页，可以从文章页返回那一天。

## 媒体读取

正文里的 `/api/media/<id>` 由 `GET /api/media/{id}` 提供：服务端核对 Owner 后，302 跳转到对象存储的签名 URL，浏览器直接从对象存储取字节。

- 签名时间对齐到整点，有效期 2 小时，所以同一小时内同一个文件的地址不变，浏览器缓存有效。
- 图片、视频、音频以 `inline` 打开，其他类型一律作为附件下载。
- 对象存储必须能被浏览器访问。服务端访问存储用的是容器内部地址时，用 `PALACE_S3_PUBLIC_ENDPOINT` 指定浏览器能访问的地址，见[环境变量](../../server/环境变量.md#媒体存储)。
