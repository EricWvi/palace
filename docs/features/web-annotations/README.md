# 原网页标注

Chromium 浏览器插件在任意网页上留下与 Palace 相同的高亮、划线、波浪线和批注，按网页地址存进 Palace。文章导入 Palace 后，在插件里把网页绑定到这篇文章；打开 Palace 文章页时，这些标注在正文里重新定位，迁移成文章标注。决策见[原网页标注 ADR](../../../specs/decisions/extension/0-web-page-annotations-bound-and-migrated-into-articles.md)与[按标签页开启 ADR](../../../specs/decisions/extension/20261021-annotating-off-until-turned-on-per-tab.md)，文章标注本身见[标注](../annotations/README.md)。

## 安装插件

1. `task build:extension`，产物在 `apps/palace-extension/dist`。
2. Chromium 打开 `chrome://extensions`，开启开发者模式，「加载已解压的扩展程序」选择 `dist`。
3. 在 Palace 的密钥管理里为这台电脑新建一个 Access Token，打开插件页（扩展程序的「选项」，或弹窗里的「标注过的网页与设置」），填入 Palace 地址和 token 后保存。保存前插件会用 token 请求一次 `GET /api/me`，Palace 不接受就不保存。

token 只保存在本机的 `chrome.storage.local`，不经浏览器同步；所有请求由插件的 service worker 带 `Authorization: Bearer` 发出，网页里的脚本拿不到 token。插件页里的 token 输入框只写不读，留空保存表示只改地址。

## 在网页上标注

- 标注默认关闭：插件不查询 Palace，不出现选区小条，不显示任何标注，也不往网页里加节点。在要标注的网页上点插件图标，选「在这个标签页开启标注」，这个标签页才开始显示原网页标注或只读标注、接受新标注。开启状态只属于这个标签页，换网页、刷新都保持，直到在弹窗里「关闭标注」或关掉标签页；它存在 `chrome.storage.session`，退出浏览器后清空。插件加载前就打开的标签页要刷新一次才生效。
- Palace 自己的页面（与设置里的 Palace 地址同源）不能开启，那里用 Palace 自己的标注。已开启的标签页导航到 Palace 页面时，插件在那一页保持不动。
- 选中文字后出现与 Palace 相同的小条：四个色点、划线、波浪线、「批注」，快捷键 `1`–`4`（`h` 同 `1`）、`u`、`w`、`c`。点击已有标注可以换样式、改批注或删除（`Delete`）；再点一次当前样式会去掉它，没有批注时整处删除。
- 悬停带批注的标注，批注浮在原文下方；点击批注可以原地修改，离开、`Esc` 或 `⌘/Ctrl+Enter` 保存。
- 小条和批注随浏览器的亮暗外观切换。
- 标注用 CSS Custom Highlight API 绘制，不改动网页 DOM；小条和批注放在 closed shadow root 里。网页加载后内容变化时，插件会在页面安静下来后重新定位。
- 标注颜色取自 `design/tokens.css`，按每处标注所在文字的颜色选亮色或暗色那一套：文字是浅色就当作暗底，用暗色 token，反之用亮色 token，并同时设定文字颜色，保证在任何网站上可读。看的是网页自己的文字颜色而不是浏览器的亮暗外观，因为网站可能无视后者；浏览器外观切换时会重画一次，网站自己的主题开关不会触发重画，要等页面内容下次变化。
- 网页地址取 `location.href` 去掉 `#` 之后的部分，不做其他规范化。每次新建、修改标注都会记下当前标签页标题，供列表页显示。
- 插件不在本地保存任何标注，开启的标签页里每次操作直接写 Palace，多台电脑之间以后写入的为准。新建和修改在等待 Palace 回复时就先画出结果，保存失败再退回原样并提示；删除等 Palace 确认后才消失，与 Palace 一致。连不上 Palace 或 token 无效时，选区小条不出现，工具栏图标显示 `!`。
- 工具栏图标随浏览器的亮暗外观切换：插件运行的页面把 `prefers-color-scheme` 报给 service worker，由它调用 `chrome.action.setIcon`。manifest 只能写一套静态图标，所以浏览器刚启动、还没打开任何网页时显示的是亮色版本。
- 只处理顶层页面正文里的文字；iframe、输入框、可编辑区域和 PDF 查看器不支持。

## 收进 Palace：绑定与迁移

1. 文章导入 Palace 之后，在原网页上点插件图标，开启标注后选「收进 Palace」。候选列表请求 `GET /api/articles?near=<网页地址>`：同一主机名（忽略 `www.` 和大小写）的排在前面，其中路径开头相同的段越多越靠前，其余按导入时间倒序；搜索框输入时加上 `q`，只过滤不改排序。每项显示标题、作者（或站点）和原文地址。
2. 选中文章后完成绑定，插件在新标签页打开这篇文章的 Palace 页面。没有标注的网页也可以绑定。
3. Palace 文章页渲染出正文后，请求 `GET /api/articles/{id}/web-annotations`，在正文的标注文字里逐条定位：引文、前后文和正文都去掉全部空白后找引文的完全匹配，按前后文吻合的长度打分，分数相同时取位置比例最接近的一处。定位成功的一次提交给 `POST /api/articles/{id}/web-annotations/migrate`，服务端在一个事务里删除这些原网页标注、新建文章标注；样式、批注和创建时间沿用原来的。
4. 定位失败的留在 meta 行「M 处原网页标注没对上」里，点开可以对照原文在正文中重新标注，或「丢弃」。正文不会变化，所以它们不会被再次尝试。

绑定之后：

- 原网页不再接受新标注，已有的也不能修改；插件改为只读显示这篇文章**当前**的标注（按引文查找，多处出现时取位置比例最接近的一处），工具栏图标显示 `✓`，弹窗里可以打开文章或「改绑到另一篇」。
- 改绑只改变剩余原网页标注之后迁往哪里，以及原网页上显示哪篇文章的标注；已经迁移的标注留在原来的文章里。
- 删除文章时，绑定到它的网页和剩余的原网页标注一起删除。未绑定网页的最后一处标注被删除时，网页记录也一起删除。

## 接口

| 方法与路径 | 作用 |
| --- | --- |
| `GET /api/web-pages` | 全部网页，按最近修改倒序，不分页；每项带 `annotation_count` |
| `GET /api/web-pages/lookup?url=` | `{ page, annotations }`；没有记录返回 404 |
| `POST /api/web-pages/annotations` | `{ url, title, quote, prefix, suffix, position, style, note }`，网页不存在时一并创建，返回 201；网页已绑定返回 409 `web_page_bound` |
| `PUT /api/web-annotations/{id}` | `{ title, style, note }`，一起替换样式和批注并记下标签页标题；网页已绑定返回 409 `web_page_bound` |
| `DELETE /api/web-annotations/{id}` | 删除，返回 204；已绑定网页上剩余的标注也可以删除 |
| `PUT /api/web-pages/binding` | `{ url, title, article_id }`，绑定或改绑，网页不存在时一并创建 |
| `GET /api/articles?near=` | 见上文；带 `near` 时游标只能用于同一种排序 |
| `GET /api/articles/{id}/web-annotations` | 绑定到这篇文章的所有网页上剩余的原网页标注 |
| `POST /api/articles/{id}/web-annotations/migrate` | `{ items: [{ web_annotation_id, start, end, quote }] }`，返回新建的文章标注；已不存在或不属于这篇文章的项跳过 |

字段规则：`url` 是绝对 `http`/`https` 地址、不超过 2048 字节、不含 `#`；`title` 为空时用地址，超过 1024 字节截断；`quote` 非空、UTF-8 不超过 16 KiB；`prefix`、`suffix` 各不超过 32 个 UTF-16 码元；`position` 在 `[0, 1]`；`style` 与 `note` 的规则同文章标注。迁移请求中任一锚点不合规则（`quote` 的 UTF-16 长度必须等于 `end - start`）时整个请求返回 400，什么都不迁移。其他 Owner 的网页、原网页标注和文章都返回 404。字段的完整定义见 [OpenAPI](../../../contracts/openapi.json)。

## 工程入口

- 数据库：`crates/db/migrations/0019_web_annotations.sql`、`crates/db/src/web_page.rs`（网页与原网页标注）、`crates/db/src/web_page/migrate.rs`（迁移事务）、`crates/db/src/article_list/near.rs`（`near` 排序）；校验与排序规则：`crates/domain/src/web_page.rs`、`crates/domain/src/web_page/nearness.rs`；接口：`crates/backend/src/http/web_pages.rs`。
- Palace 文章页：`apps/palace-web/src/lib/text-quote.ts`（定位纯函数，插件共用）、`apps/palace-web/src/components/web-annotations.tsx`（迁移与「没对上」列表）。
- 插件：`apps/palace-extension`。`src/palace.ts` 是 service worker 中唯一持有 token 的请求处理，`src/protocol.ts` 定义各部分之间的消息；`src/content.ts`、`src/page-text.ts`、`src/highlights.ts`、`src/overlay.ts` 是网页上的标注；`src/popup.ts` 与 `src/pages.ts` 是弹窗和插件页。浏览器回归测试 `npm run test:browser -w apps/palace-extension` 会先构建插件，再在真实 Chromium 中加载。
