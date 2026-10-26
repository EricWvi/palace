---
status: implemented
date: 2026-10-10
---

# Markdown 展示抽成全站共用组件：代码块按标注语言高亮、悬停复制，图片按调用方规则加载

> 2026-10-26 增补：D6 公式、D7 mermaid 图表，二者都是不可标注的整体；D1–D5 不变。

> 历史决策：D5 中 `load` 规则的 `resolve` 只返回地址；[文章决策](../../web/stars/20261020-articles-imported-with-media.md) D7 改为返回 `{ kind: "image" | "video", src }`，以便文章嵌入视频。其余规则不变。

阅读页正文里标注了语言的 Markdown 代码块按该语言语法高亮，配色只区分五类语法成分，颜色取自 Palace 的暖色 token，亮暗各一套。没写语言或标注为 `text` 的代码块保持纯文本，不自动猜测语言。每个代码块右上角有“复制”按钮：指针悬停到代码块或键盘焦点进入时出现，平时不显示；没有悬停能力的设备上一直以淡色显示。点击后复制代码原文，按钮文字变为“已复制”，约 1.5 秒后恢复。这套展示由全站共用的 `MarkdownContent` 组件提供，对话、笔记、文章、卡片笔记都用它；各处唯一的差别是图片能否加载，由调用方以图片规则声明（D5）。

当前为 `implemented`，核心测试用例见[Markdown 展示](../../../test-cases/server/conversation/markdown-content.md)。涉及 Web 端的 `MarkdownContent` 组件、阅读页对它的使用、样式、设计 token 与打包分组；服务端、接口、存储、时间线摘录与消息目录行都不变。决策起于对话阅读页，因此放在本目录；笔记、文章的决策引用 D5 而不另立 Markdown 规则。高亮只改变展示，不改写、不规范化 `content`。

## 继承与修改

继承[对话在摘星阅读页阅读与管理](20261004-reading-page-owns-conversation-actions.md) D1 的正文排版，只扩展其中的 Markdown 展示规则；消息目录见[阅读页消息目录](20261006-message-toc-on-the-reading-page.md)，不受影响。

| 前序约定 | 本决定 |
| --- | --- |
| Markdown 展示规则：GFM、不执行原始 HTML、不加载远程图片、链接在新窗口打开 | 继承，不变；增加代码块高亮与复制，见 D1–D4 |
| 正文使用内容字体，代码块为等宽字体加浅色底块 | 继承；底块内的文字按 D3 上色 |
| user 消息放在浅色底块中，assistant 不加底色 | 继承，不变；两种消息里的代码块规则相同 |
| 时间线摘录与目录行由服务端从 `content` 纯文本化生成 | 继承，不变；高亮不进入这两处 |
| `content` 原样保存，展示时安全渲染 | 继承，不变；高亮只生成带类名的 `span`，不引入 HTML 执行 |
| 阅读页消息中不加载图片、所有链接在新窗口打开 | 继承：对话以 `placeholder` 图片规则调用；修改：站内路径链接改为当前页内跳转，见 D5 |

## D1：用 highlight.js 的常见语言集在渲染时同步高亮

通过 `rehype-highlight`（底层为 highlight.js 的 `lowlight`）作为 `react-markdown` 的 rehype 插件，载入 `lowlight` 的常见语言集（约 37 种），覆盖 JavaScript/TypeScript（含 `jsx`、`tsx`）、Python、Rust、Go、Java、Kotlin、C/C++/C#、Swift、PHP、Ruby、Bash/Shell、SQL、JSON、YAML、INI、HTML/XML、CSS/SCSS/Less、Markdown、Diff、GraphQL、Makefile 等；语言名的别名以该语言集的定义为准，例如 `js`、`html`、`sh`、`yml` 都能识别。

为什么是 highlight.js 而不是 Shiki：

- 高亮在渲染时同步完成。Shiki 需要异步加载语法与主题，长对话打开或滚动时会先显示纯文本再变成高亮，正文出现跳变。
- highlight.js 只给代码片段加类名，颜色由 Palace 的 CSS 决定，直接跟随 token 和暗色模式。Shiki 的优势是 VS Code 级别的语法精度与几十种语法成分，而 D3 只区分五类成分，这项优势用不上。
- 高亮实际落在标注了语言的代码块上（D2），来源导出中主要是 `js`、`html`、`css`，highlight.js 对这些语言的效果足够。

高亮库打成单独的代码包，gzip 后约 54 KB，主要是 37 种语法定义，打包分组见 D5。首页不加载它；若以后需要压缩，可以改为只注册部分语言，不影响本决策的其他规则。语言集是可替换的插件配置，以后换库只改前端，不涉及数据和接口。

## D2：只高亮标注了已知语言的代码块

| 代码块 | 结果 |
| --- | --- |
| 标注了语言集中的语言（含别名） | 按该语言高亮 |
| 没写语言 | 纯文本 |
| 标注为 `text`、`txt`、`plaintext` | 纯文本 |
| 标注了语言集之外的语言 | 纯文本，不报错、不提示 |
| 行内代码 `` `code` `` | 不高亮，样式不变 |

不开启自动识别语言。来源导出的样例中约一半代码块没写语言，另有约四分之一标注为 `text`，看上去收益很大；但自动识别对短代码、命令输出和日志经常判错，把日志上成 SQL 色比不上色更误导；它还要对每个代码块逐一尝试几十种语法，长对话的渲染开销随代码块数量增长。来源已经写明的语言是唯一可靠的依据。

## D3：配色只区分五类语法成分

| 成分 | highlight.js 类名 | 处理 |
| --- | --- | --- |
| 关键字 | `hljs-keyword`、`hljs-built_in`、`hljs-selector-tag`、`hljs-name`（标签名） | 关键字色 |
| 字符串 | `hljs-string`、`hljs-regexp`、`hljs-attr`、`hljs-attribute`、`hljs-symbol` | 字符串色 |
| 注释 | `hljs-comment`、`hljs-quote`、`hljs-meta` | 与 `--text-3` 同级的弱化色 |
| 数字和字面量 | `hljs-number`、`hljs-literal` | 字面量色 |
| 函数名和类型名 | `hljs-title`（含函数、类）、`hljs-type` | 名称色 |

其余类名（变量、运算符、标点等）使用正文颜色，不加粗、不加斜体、不加背景。

在 `design/tokens.css` 新增 `--code-keyword`、`--code-string`、`--code-comment`、`--code-literal`、`--code-name` 五个 token，亮色与暗色各定义一套，从现有暖色体系取色（关键字靠近 `--accent`，字符串靠近 `--success`）。每个颜色在代码块底色 `--bg-sunken` 上的对比度不低于 WCAG AA 的 4.5:1。

为什么克制：阅读页以文字为主，代码块是正文的一部分，不是编辑器。五类成分足以让结构一眼可辨；更细的区分要更多颜色，彩色主题会压过正文，也难与暖色配色协调。

## D4：复制按钮

每个代码块（无论是否高亮）右上角有一个文字按钮“复制”，无障碍名称为“复制代码”。

| 情形 | 显示 |
| --- | --- |
| 有悬停能力的设备，指针不在代码块上 | 不显示 |
| 指针悬停到代码块上，或键盘焦点进入按钮 | 显示 |
| 没有悬停能力的设备（`@media (hover: none)`） | 一直显示，用淡色 |

- 复制的内容是代码块的原始文本：与 `content` 中代码围栏内的文字一致，不含高亮标记，去掉 Markdown 解析附带的末尾一个换行。
- 写入剪贴板成功后，按钮文字变为“已复制”，约 1.5 秒后恢复为“复制”；失败时显示“复制失败”，同样恢复。文字变化通过 `aria-live` 告知屏幕阅读器。不弹提示框。
- 按钮固定在代码块可视区域的右上角，代码横向滚动时不随内容移动，也不遮住第一行文字的开头。

为什么平时隐藏：一页长对话可能有几十个代码块，常驻按钮会在正文中形成一列重复控件，违背少 UI 的原则；需要复制时指针已经在代码块上。键盘焦点进入时显示，保证不用鼠标也能找到。Palace Web 目前主要在 PC 上使用，移动端以后由原生 app 承担；没有悬停能力时一直淡色显示，只是避免按钮永远不可达，不针对移动端另做设计。

## D5：全站共用 `MarkdownContent`，图片规则由调用方声明

Markdown 的展示不再属于阅读页消息，而是 Web 端共用组件 `MarkdownContent`。对话、笔记、文章、卡片笔记都通过它展示 Markdown；GFM、不执行原始 HTML、D1–D4 的高亮与复制、链接规则在各处相同，只有图片规则由调用方声明：

```ts
type ImagePolicy =
  | { kind: "placeholder" }
  | { kind: "load"; resolve: (src: string) => string | null };
```

| 规则 | 行为 | 使用者 |
| --- | --- | --- |
| `placeholder` | 不发起任何图片请求，显示“[图片：alt]” | 对话：来源导入的第三方内容 |
| `load` | 只加载 `resolve` 返回的地址；返回 `null` 时显示占位。加载的图片带 `loading="lazy"` 与 `referrerpolicy="no-referrer"` | 笔记、文章、卡片笔记：图片由主人上传到 Palace |

笔记图片上传到 Palace、存放在 MinIO（S3 兼容存储），沿用 legacy dashboard 的做法：正文保存稳定的应用地址，服务端按 Owner 校验后跳转到签名地址，签名过期由服务端续期而不改写正文。因此笔记的 `resolve` 只接受 Palace 附件地址，外站地址返回 `null`；附件地址格式与服务端接口由笔记决策定义。

为什么用带数据的联合类型而不是布尔开关：允许加载必须同时说明哪些地址可加载，`{ kind: "load", resolve }` 让二者不可分离；对话的调用写成 `{ kind: "placeholder" }`，读调用处即可知道不会请求任何图片。图片规则之外的规则不开放成选项：它们在各处应当一致，开放后各页会逐渐分叉。

链接规则：以单个 `/` 开头的站内路径通过前端路由在当前页跳转，其余链接（含 `//host` 协议相对地址）在新窗口打开并带 `rel="noreferrer"`。笔记会链接到对话与其他笔记，站内跳转不应打开新窗口；导入的对话几乎不含站内路径，行为不受影响。

样式挂在组件自己的 `.markdown` 类下，阅读页 `.body` 只保留消息容器的样式（user 消息的底块与间距）。组件按 `content` 与图片规则记忆化，调用方传入稳定的规则对象。

打包时 `rehype-highlight`、`lowlight`、`highlight.js` 固定分入 `highlight` 组。它们占 Markdown 展示的大部分体积、只随依赖升级而变；单独成包后，阅读页、笔记页各自发版都不会让读者重新下载它，各页共用同一份缓存。页面静态引用它，它与页面代码并行下载，高亮仍在首次渲染时完成。卡片笔记不出现在时刻页，首页不需要它。

## D6：`$` 与 `$$` 公式用 KaTeX 在渲染时同步排版

`remark-math` 识别公式，`rehype-katex` 排版：`$…$` 为行内公式，`$$…$$` 与 ` ```math ` 为独立公式。写错的公式不报错，以 KaTeX 的错误色显示原文。独立公式过宽时自己横向滚动，不撑宽正文栏。

KaTeX 与 highlight.js 一样同步输出 HTML，公式在首次渲染时就位，满足不变量 3 的同一要求；MathJax 需要异步排版，正文会跳变。`rehype-katex` 排在高亮之前，高亮插件看不到它替换掉的 `language-math` 代码。KaTeX 固定分入打包分组 `katex`，理由同 D5 的 `highlight` 分组。

单个 `$` 也作公式分隔符，笔记里的 `$x$` 写法才能显示；代价是正文里同一段中出现两个美元金额（如 `$5 和 $10`）会被当成公式。笔记与模型回答里公式远多于成对的美元金额，接受这一代价。

公式整体带 `data-annotation-skip`，不计入标注文字（见[摘星标注](../../web/stars/20261021-annotations-on-articles-and-messages.md) D3 的坐标规则）：KaTeX 输出的字形文字（MathML 与 HTML 两份）不是读者能选作引文的文字，在它的排版 `span` 中间插入 `mark` 还会弄坏公式。

## D7：` ```mermaid ` 代码块画成图表，点击放大

标注为 `mermaid` 的代码块画成图表，不高亮、没有复制按钮：

| 情形 | 显示 |
| --- | --- |
| mermaid 未加载完 | 占位空白 |
| 解析成功 | 图表按正文栏宽缩放；点击后在对话框中按原始尺寸显示，过大时可滚动 |
| 解析失败 | 原文，等宽字体，不高亮 |

- mermaid 体积远大于页面其余部分，只在第一次遇到图表时动态加载，不进入任何固定分组；它自带的 KaTeX 副本也留在它自己的包中。图表因此是异步出现的，这是对不变量 3 的有意例外：同步加载会让所有不含图表的页面都下载 mermaid。
- 以 `securityLevel: "strict"` 绘制：对话是来源导入的第三方内容，`strict` 转义标签文字并禁用点击回调，满足不变量 7。
- 用 mermaid 的 `base` 主题，颜色和字体在绘制时从页面 token（`--bg`、`--bg-sunken`、`--text`、`--text-3`、`--border`、`--font-ui`）读取；系统亮暗切换时重画。mermaid 自带主题的冷灰与蓝色和暖色页面不协调；节点投影关闭，页面以文字为主，不加装饰。解析失败时关闭 mermaid 自己的错误图（`suppressErrorRendering`），否则它会残留在页面末尾。
- 图表没有边框和按钮，点击即放大；入口是图表本身，符合少 UI 的原则。

图表与 D6 的公式一样整体带 `data-annotation-skip`，解析失败显示的原文也是；图表文字是 SVG 中的标签，不是正文。

D6、D7 改变了 `MarkdownContent` 的文字输出：此前公式源码、`$` 与 mermaid 源码都计入标注文字。之后的已有锚点会平移，由标注 D3 的 `quote` 兜底重新定位；引文本身含公式源码或 mermaid 源码的旧标注不再绘制。来源导出与笔记中这类标注极少，不另写迁移。

## 为什么不是这些替代方案

| 替代方案 | 优势 | 取舍 |
| --- | --- | --- |
| Shiki | 语法精度最高，主题丰富 | 异步加载导致正文跳变；细粒度成分与主题在五类配色下用不上（D1、D3） |
| Prism（`refractor`） | 与 highlight.js 相近 | 没有可以落到本决策约束上的优势，`rehype-highlight` 与 `react-markdown` 的集成更直接 |
| 自动识别没写语言的代码块 | 一半代码块也能上色 | 短代码、日志常被判错；长对话渲染开销随代码块数量增长（D2） |
| 使用现成彩色主题（GitHub、One Dark 等） | 不用自己调色 | 与暖色 token 和暗色模式脱节，颜色压过正文（D3） |
| 复制按钮常驻 | 一眼可见 | 长对话中形成大量重复控件（D4） |
| 加语言标签 | 读者知道是什么语言 | 本次不需要；高亮本身已提示语言，且会增加每个代码块的常驻 UI |
| 每页各自写 Markdown 组件 | 各页可按需定制 | 高亮、复制、安全规则会逐渐分叉，同一段内容在对话与笔记里显示不同（D5） |
| 图片用布尔开关 `allowImages` | 接口最简单 | 无法表达可加载的地址范围，笔记会连带加载外站图片（D5） |
| MathJax | 支持的 LaTeX 更全，输出可访问性更好 | 异步排版，正文跳变（D6）；KaTeX 覆盖笔记与模型回答中的常用写法 |
| 只认 `$$`、不认单个 `$` | 美元金额不会误判 | 笔记里通行的 `$x$` 写法无法显示（D6） |
| 公式与图表计入标注文字 | 读者可以标注公式 | 字形文字不是可读的引文，`mark` 会弄坏 KaTeX 排版（D6、D7） |
| mermaid 进入固定分组、同步加载 | 图表不异步出现 | 每个 Markdown 页面都多下载数百 KB，绝大多数页面没有图表（D7） |
| 高亮库留在各页面代码包中 | 不需要打包配置 | 任何一页发版都让读者重下 54 KB；多页各打一份或共享包随页面代码变化（D5） |

## 不变量

1. 只有标注了语言集中语言的代码块被高亮；没写语言、`text` 类与未知语言的代码块保持纯文本。
2. 高亮不改变代码块的文字内容，`content` 与服务端数据不受影响。
3. 高亮在首次渲染时完成，正文不出现从纯文本到高亮的跳变。
4. 代码文字最多使用五种语法颜色加正文颜色，全部来自设计 token，亮暗模式各自满足 4.5:1 对比度。
5. 每个代码块都有复制按钮；有悬停能力时只在悬停或获得焦点时显示，没有悬停能力时一直淡色显示。
6. 复制得到的文本与代码围栏内的原文一致。
7. 原始 HTML 不执行。
8. 每处 Markdown 展示都经过 `MarkdownContent`，代码高亮、复制按钮与链接规则在各处一致。
9. `placeholder` 图片规则下不发起任何图片请求；`load` 规则下只加载 `resolve` 返回的地址，并且懒加载、不带来源信息。
10. 高亮库只在打包分组 `highlight` 中，首页不加载它，它的文件名只随依赖版本变化。
11. 公式在首次渲染时排版；KaTeX 只在打包分组 `katex` 中，首页不加载它。
12. mermaid 只在出现图表时加载，以 `strict` 安全级别绘制；解析失败时显示原文。
13. 公式与图表整体带 `data-annotation-skip`，标注文字在 hast 与 DOM 两侧都不计入它们。

## 本决策未解决的问题

- **语言标签、行号、长代码块折叠**：本次不做，需要时另行决定。
- **原生 app 中的代码块展示**：由原生 app 的决策决定，本决策只约束 Web。
- **笔记图片的上传、附件地址格式与签名续期**：由笔记的决策定义；`MarkdownContent` 只通过 `resolve` 接受其结果。
- **笔记编辑器中的代码高亮**：所见即所得编辑器若选用基于 lowlight 的方案，应复用 `highlight` 分组与 D3 配色，由笔记决策确认。

## 落地与验收

`components/markdown-content.tsx` 提供 `MarkdownContent` 与 `ImagePolicy`：`rehype-highlight` 以 `detect: false` 与 `plainText: ["text", "txt", "plaintext"]` 配置，未注册的语言由插件跳过；`pre` 被替换为带定位容器的代码块，复制按钮在 `pre` 之外，读取 `pre` 的文本并去掉末尾一个换行后写入剪贴板，状态文字经 `role="status"` 告知辅助技术；站内路径用 React Router 的 `Link`。阅读页以模块常量 `{ kind: "placeholder" }` 调用。五个 `--code-*` token 定义在 `design/tokens.css` 的亮色与两处暗色块中；`vite.config.ts` 以 `codeSplitting.groups` 声明 `highlight` 分组。生产构建中首页只加载原有代码包，打开阅读页时 `highlight` 包与阅读页代码包并行加载。

公式与图表（D6、D7）：`rehypePlugins` 依次为 `rehype-katex`、高亮与 `rehypeAtomicBlocks`，后者给 KaTeX 输出（`katex`、`katex-display`、`katex-error`）和 `mermaid` 代码块的 `pre` 打上 `dataAnnotationSkip`；`pre` 遇到 mermaid 时交给 `components/mermaid-diagram.tsx`，放大用共用的 `Dialog`。`vite.config.ts` 的 `katex` 分组排除 mermaid 嵌套的 KaTeX 副本。`lib/annotation-paint.ts` 的 hast 计数跳过带 `dataAnnotationSkip` 的元素。

组件测试覆盖高亮范围、文字不变、原始 HTML 不执行、复制原文与两种反馈的恢复、两种图片规则与链接规则、公式排版与出错显示、图表绘制、放大与解析失败时显示原文；标注测试覆盖公式与图表前后的锚点在两侧计数一致；Chromium 测试覆盖亮暗模式下五类颜色取自 token 且对比度达标、悬停与键盘聚焦时显示、横向滚动时按钮不动、触屏环境常显淡色。
