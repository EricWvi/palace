# Markdown 展示核心测试用例

决策：[Markdown 展示抽成全站共用组件](../../../decisions/server/conversation/20261010-shared-markdown-content-with-code-highlighting.md)，继承[对话阅读页](reading-page.md)的正文排版与 Markdown 安全展示规则。

## Only code blocks labeled with a known language may be highlighted

风险：没写语言或标注为 `text` 的日志、命令输出被猜成某种语言并错误上色；未知语言让渲染报错或整条消息消失；高亮改变代码文字；先显示纯文本再变成高亮。前置：一条消息含 `js`、`html`、`tsx` 别名、没写语言、`text`、未知语言（如 `brainfuck`）的代码块各一个，以及行内代码。触发：打开阅读页。

必须成立：首次渲染时已知语言（含别名）的代码块带 highlight.js 语法类名；没写语言、`text` 类与未知语言的代码块只含原文文字、没有语法类名；行内代码不高亮；每个代码块的文字与围栏内原文一致；原始 HTML 仍不执行。禁止自动识别语言，禁止未知语言导致错误。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 已知语言与别名在首次渲染时高亮 | Covered | `components/markdown-content.test.tsx::highlights only blocks labeled with a known language and keeps every text as written`（同步渲染后立即断言类名） |
| 没写语言、`text` 类、未知语言、行内代码保持纯文本，文字与原文一致 | Covered | `components/markdown-content.test.tsx::highlights only blocks labeled with a known language and keeps every text as written`（含原始 HTML 不执行） |

决策依据：D1、D2，不变量 1–3、7。

## Syntax colors must come from the five code tokens in both themes

风险：配色压过正文、在暗色模式下不可读，或硬编码颜色脱离设计 token。前置：一个包含关键字、字符串、注释、数字、函数名的已知语言代码块。触发：分别在亮色和暗色模式下打开阅读页。

必须成立：五类成分分别使用 `--code-keyword`、`--code-string`、`--code-comment`、`--code-literal`、`--code-name` 的当前主题值，其余文字使用正文颜色；每个颜色在 `--bg-sunken` 上对比度不低于 4.5:1。禁止加粗、斜体或背景色。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 五类成分在亮暗模式下取 token 值，且无加粗、斜体、背景 | Covered | `e2e/code-blocks.spec.ts::syntax colours come from the code tokens and stay readable in both themes`（真实 Chromium） |
| 五个 token 亮暗两套都满足 4.5:1 | Covered | `e2e/code-blocks.spec.ts::syntax colours come from the code tokens and stay readable in both themes`（真实 Chromium，按计算后的颜色求对比度） |

决策依据：D3，不变量 4。

## The copy button must copy the original code and stay out of sight until wanted

风险：长对话中常驻按钮形成重复控件；键盘用户找不到按钮；复制到高亮标记、多余换行或其他代码块的内容；失败时没有反馈；横向滚动时按钮随内容移走或遮住代码。前置：一条消息含两个代码块，其中一个很宽需要横向滚动。触发：指针移入移出代码块，用 Tab 聚焦按钮，点击复制（成功与剪贴板拒绝两种情形），横向滚动宽代码块；在没有悬停能力的环境下打开。

必须成立：有悬停能力时按钮只在悬停或聚焦时可见；没有悬停能力时一直淡色可见；复制内容等于该代码块围栏内原文、去掉末尾一个换行；成功显示“已复制”、失败显示“复制失败”，约 1.5 秒后恢复“复制”，变化可被屏幕阅读器读到；横向滚动时按钮位置不变。禁止弹出提示框。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 复制原文、反馈文字与恢复 | Covered | `components/markdown-content.test.tsx::copies the fenced original, says so, and falls back after a moment`（userEvent 剪贴板桩，含拒绝情形） |
| 悬停或聚焦时显示、平时隐藏、无悬停能力时常显 | Covered | `e2e/code-blocks.spec.ts::the copy button appears on hover or keyboard focus and holds its corner while code scrolls`（真实 Chromium，含触屏上下文） |
| 横向滚动时按钮固定在右上角 | Covered | `e2e/code-blocks.spec.ts::the copy button appears on hover or keyboard focus and holds its corner while code scrolls` |

决策依据：D4，不变量 5、6。

## Images must load only from addresses the caller resolves

风险：导入的对话让读者浏览器请求第三方图片（追踪像素、来源失效后的报错）；笔记连带加载外站图片；加载的图片一次性全部请求或向图片所在站点透露来源页面。前置：一段 Markdown 含 Palace 附件地址的图片、外站图片与无 alt 的图片。触发：分别以 `placeholder` 与只接受附件地址的 `load` 规则展示。

必须成立：`placeholder` 下没有任何 `img` 元素，每张图片显示“[图片：alt]”（无 alt 时为“未加载”）；`load` 下只有 `resolve` 返回的地址成为 `img`，带 `loading="lazy"` 与 `referrerpolicy="no-referrer"`，其余显示占位。禁止 `placeholder` 下出现图片请求。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 两种规则下的图片与占位集合、加载属性 | Covered | `components/markdown-content.test.tsx::loads images only where the caller resolves them, lazily and without a referrer` |
| 对话阅读页使用 `placeholder` | Partial | 阅读页以模块常量传入 `placeholder`，`placeholder` 分支由组件测试直接证明；尚无阅读页层面渲染含图片消息的测试 |

决策依据：D5，不变量 8、9。

## In-app links must stay in the page and others must open a new window

风险：笔记里指向对话或其他笔记的链接弹出新窗口；外站或协议相对地址在当前页打开，离开 Palace。前置：一段 Markdown 含 `/conversations/…`、`https://…` 与 `//host` 三种链接。触发：展示并点击站内链接。

必须成立：站内路径没有 `target`，点击后由前端路由在当前页跳转；其余两种带 `target="_blank"` 与 `rel="noreferrer"`。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 三种链接的打开方式与站内跳转 | Covered | `components/markdown-content.test.tsx::follows in-app paths in place and opens other links in a new window` |

决策依据：D5，不变量 8。

## Formulas and diagrams must render and stay out of annotations

风险：公式以 `$` 源码原样显示；写错的公式让整条消息渲染失败；mermaid 代码块只显示源码，或以宽松安全级别绘制第三方内容；解析失败的图表消失；公式或图表的字形文字计入标注坐标，使 hast 与 DOM 两侧计数不一致，后面的标注落到别处，或 `mark` 插进 KaTeX 排版。前置：一段 Markdown 含行内公式、独立公式、写错的公式、一个可解析与一个解析失败的 mermaid 代码块，以及公式、图表前后各有相同文字。触发：展示，点击图表，在公式与图表之后的文字上标注。

必须成立：行内公式渲染为 `.katex`、独立公式为 `.katex-display`，写错的公式显示为带原文的 `.katex-error`；可解析的 mermaid 块以 `strict` 安全级别、`base` 主题画成可点击放大的图表，放大后的副本使用独立 id 与原始尺寸；解析失败的块显示原文；图表块没有复制按钮；公式、图表与失败原文都带 `data-annotation-skip`，标注文字中不含它们，其后的标注落在所选的那一处，公式与图表内部没有 `mark`。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 公式排版、出错显示原文、整体带跳过标记 | Covered | `components/markdown-content.test.tsx::typesets $ and $$ formulas with KaTeX and keeps them out of the annotation text`（jsdom 中真实 KaTeX） |
| 图表绘制参数、放大副本、解析失败显示原文、无复制按钮 | Covered | `components/markdown-content.test.tsx::draws mermaid fences as diagrams that enlarge, and shows the source when one does not parse`（mermaid 为桩，jsdom 无法排版 SVG） |
| 真实 mermaid 在亮暗模式下按页面 token 取色、放大、解析失败时不在页面留下错误图 | Covered | `e2e/math-and-diagrams.spec.ts::real mermaid draws in the page's colours in both themes and enlarges on click`（真实 Chromium） |
| 公式与图表前后 hast 与 DOM 计数一致，标注不落入其中 | Covered | `lib/annotation-paint.test.tsx::leaves formulas and diagrams out of the annotation text`（jsdom 中真实 `MarkdownContent` 与 KaTeX，mermaid 为桩） |

决策依据：D6、D7，不变量 11–13。
