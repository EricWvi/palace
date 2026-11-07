---
status: implemented
date: 2026-11-07
---

# 导出文章为 `pal article upload` 读取的目录

文章页 `···` 菜单加“导出文章”：下载 `<标题>.zip`，解压后是 `pal article upload` 读取的目录，含元数据、正文和正文嵌入的全部媒体。

当前为 `implemented`，核心测试用例见[摘星文章](../../../test-cases/web/stars/articles.md#exporting-an-article-must-write-the-directory-pal-uploads)。继承[文章通过 API 连同媒体一次导入](20261020-articles-imported-with-media.md)除 D6 菜单项以外的全部决策，沿用[标题菜单加“导出分支”](../../server/conversation/20261022-export-branch-from-title-menu.md)“导出格式即导入格式”的原则，改变其“在浏览器内生成”的做法。

## 继承与修改

| 前序约定 | 本决定 |
| --- | --- |
| 文章决策 D6：`···` 菜单有“编辑文章”和“删除文章” | 修改：在二者之前加“导出文章”，见 D1 |
| 导出分支 D2：导出格式与导入格式相同 | 继承：导出写上传目录的结构，见 D2 |
| 导出分支 D2：导出在浏览器内完成，服务端接口不变 | 不适用：文章由服务端打包，见 D3 |
| 媒体读取决策：bucket 不需要公开读，也不需要 CORS | 继承，这正是 D3 的原因 |

## D1：菜单位置

“导出文章”不改数据，排在“只看标注”之后、“编辑文章”之前，不可撤销的“删除文章”仍在最后，顺序与对话标题菜单相同。

## D2：导出格式是上传目录

压缩包含 `article.json`、`article.md` 与 `assets/<导入时的文件名>`。`article.json` 含 `title`、`url`、`date`、`author`，取当前元数据，`date` 用 `GET /api/articles/{id}` 的写法。

正文里地址属于本文媒体的内联图片改回 `assets/<文件名>`，其他字节不变，判定规则与导入时识别 `media:` 引用相同。导入时正文里本来就写着、不属于本文的 `/api/media/<id>` 也原样保留，这样的文章要手工处理后才能再次上传。标注、Moment 时间和媒体 ID 不导出：上传目录里没有它们，导出也不另立一种格式。

**为什么不是一个 Markdown 文件**：文章导入时保存媒体，是为了此后不依赖原站；只导出文字会丢掉这一点，`/api/media/<id>` 离开 Palace 也打不开。

## D3：由服务端打包

`GET /api/articles/{id}/export` 返回 `application/zip`。浏览器不能打包：媒体跳转到对象存储的签名地址，bucket 没有 CORS，页面读不到字节。为导出给 bucket 加 CORS 会让部署多一项必须配对的配置，所以不这样做。

媒体先全部读入内存，读到才开始写压缩包，所以任一对象读取失败时整次返回 503，不产生缺文件的压缩包。内存上限由导入时的 `PALACE_ARTICLE_MAX_BYTES` 决定，默认 256 MiB；Palace 是单用户部署，不值得为此做流式写入。媒体不再压缩，接口也挂在 HTTP 压缩层之外，因为图片和视频压不动。

前端用带 `download` 属性的链接交给浏览器下载，大文件不进页面内存；失败时显示为浏览器的下载失败，页面不跳转。

## 不变量

1. 文章菜单的四项及其顺序固定为“只看标注”“导出文章”“编辑文章”“删除文章”。
2. 导出的压缩包要么含正文嵌入的全部媒体，要么不产生。
3. 压缩包解压后的目录结构与 `pal article upload` 读取的目录相同。
