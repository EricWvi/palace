# 回响页核心测试用例

当前决策：[回响页](../../../decisions/web/echo/0-jotting-wall-with-daily-recall-pile.md)、[瀑布流的札记点时间放大](../../../decisions/web/echo/20261101-zoom-wall-jottings-by-their-time-and-load-palace-media.md)。服务端读取接口的承诺见[札记读取接口](jotting-reading.md)；本文件只记录页面行为。

## Echo navigation must open the wide jotting page

风险：回响仍不可点击，或点击后进入错误的地址；回响页没有加宽，或加宽泄漏到其他页面；导航当前项错乱。前置：已登录，读取接口返回若干札记。触发：点击顶部导航的「回响」；从回响点击「时刻」；直接打开 `/jottings`。

必须成立：「回响」是指向 `/jottings` 的链接，回响页上只有回响带 `aria-current="page"`；回响页的页面容器最大宽度约 1200px，离开后恢复为 760px；时刻、摘星的行为不变。禁止加宽样式作用于其他页面。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 回响指向 `/jottings`，只有回响是当前项 | Covered | `app.test.tsx::marks only 时刻, keeps other sections out of reach, and links cards to the reading page`、`pages/echo.test.tsx::widens the sheet for 回响 only and marks 回响 as the current section` |
| 只有回响页加宽，离开后恢复 | Partial | `pages/echo.test.tsx::widens the sheet for 回响 only and marks 回响 as the current section`（页面容器的 `wide` 类）；实际宽度由 CSS 决定，jsdom 不计算布局 |

决策依据：D1，Web 导航根决策 D1 的修改项。

## Read recall cards must leave the pile for the page session

风险：关闭放大的卡片后它仍留在卡片组中；已读状态被写进 `localStorage` 或服务端，刷新后不恢复；收起平铺卡片组的点击同时触发了瀑布流中的标签或“展开”；筛选时卡片组仍然占据顶部。前置：回顾接口返回 5 张札记；瀑布流中有可点击的标签。触发：点击卡片组使其平铺；点击瀑布流上的标签位置收起；再次平铺并逐张放大、关闭；读完全部 5 张；重新挂载页面；按标签筛选后再清除。

必须成立：收起卡片组的点击不改变筛选；关闭的卡片缩回原位并淡出，直到移除都不再以全尺寸出现；每关闭一张，卡片组少一张，其余顺序不变；全部读完后显示“今天的回响都已看过”；重新挂载后卡片组恢复为接口返回的 5 张；有筛选或搜索时卡片组收起，清除后恢复，已读的卡片不回来。禁止把已读状态写入任何持久存储。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 关闭即离开卡片组，读完显示提示，重新挂载后恢复 | Covered | `pages/echo.test.tsx::lets each read recall card leave the pile until the page is opened again`（同时断言没有写入 `localStorage`） |
| 关闭的卡片缩回原位并淡出，被移除前不会回到屏幕中央闪现（真实 Chromium） | Covered | `e2e/echo.spec.ts::a closed recall card fades out at its slot and never flashes back at full size`（逐帧记录不透明度；去掉 `fill: "forwards"` 时稳定失败） |
| 收起卡片组的点击被拦截，不触发瀑布流操作 | Covered | `pages/echo.test.tsx::lets each read recall card leave the pile until the page is opened again` |
| 筛选与搜索时卡片组收起，清除后恢复 | Covered | `pages/echo.test.tsx::lets each read recall card leave the pile until the page is opened again`（标签筛选与搜索使卡片组所在抽屉变为 inert，清除后恢复） |

决策依据：D3，不变量 3、8。

## A wall jotting must open in full by its time and come back as it was

风险：点击正文或空白也会放大，选字、点链接时误开；超长札记在墙上原地展开，拉长所在的列；放大期间的 Escape 同时清掉了标签筛选；关闭后原卡片不见了，或展开状态被改变。前置：瀑布流中有一张近期札记、一张展开后约 20 行的札记和一张超过 40 行的札记；当前按标签筛选。触发：点击卡片正文；点击页脚的时间；按 Escape；点击两张长札记的「展开」。

必须成立：点击正文不放大，点击时间才放大；放大期间原卡片隐藏，关闭后重新显示在原位；Escape 只关闭放大卡片，标签筛选不变；不超过 40 行的札记原地展开，超过 40 行的札记点「展开」直接放大，墙上的卡片保持折叠。禁止瀑布流卡片的时间带悬停提示；禁止放大卡片显示相对时间。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 只有时间是放大入口，放大卡片显示完整时间，瀑布流时间没有提示 | Covered | `pages/echo.test.tsx::reads a wall jotting in full from its time, with its Palace pictures, and hands it back` |
| Escape 只关闭放大卡片，原卡片回到墙上 | Covered | `pages/echo.test.tsx::reads a wall jotting in full from its time, with its Palace pictures, and hands it back` |
| 40 行以内原地展开，超过 40 行展开即放大 | Partial | `pages/echo.test.tsx::opens a body in place up to forty lines and reads a longer one in full`（jsdom 没有布局，正文高度与行高由测试替换） |
| 回顾卡片仍然关闭即读过 | Covered | 见 [Read recall cards must leave the pile for the page session](#read-recall-cards-must-leave-the-pile-for-the-page-session)，回顾与瀑布流共用同一个放大卡片 |

决策依据：20261101 决策 D1–D3、D5，不变量 1–3、7。

## Only a jotting read in full may load pictures, and only Palace media

风险：缩略卡片请求图片，图片晚到后打乱瀑布流各列；打开旧札记时浏览器请求了外站图片；手机截图按卡片宽度显示，高过一屏；卡片按打开时测得的高度固定，图片加载后只能在框内滚动。前置：札记正文同时引用 `/api/media/<uuid>` 与外站图片；媒体响应晚于放大动画结束。触发：在瀑布流中查看卡片；点击时间放大；等待图片到达。

必须成立：瀑布流卡片中两张图片都显示占位，不请求图片；放大卡片只加载 `/api/media/<uuid>`，外站地址显示占位；图片显示高度不超过 360px；图片到达后放大卡片变高，不超过视口的 80%，并保持垂直居中。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 缩略卡片只显示占位，放大卡片只加载 Palace 媒体 | Covered | `pages/echo.test.tsx::reads a wall jotting in full from its time, with its Palace pictures, and hands it back` |
| 图片限高 360px，卡片随图片居中长高且不超过视口 80%（真实 Chromium） | Covered | `e2e/echo.spec.ts::a jotting read in full grows evenly for a late tall picture and caps its height`（媒体响应被扣住到卡片放大完毕之后） |

决策依据：20261101 决策 D4、D5，不变量 4–6。

## Tag filters and search must stay visible and intersect

风险：当前筛选的标签不在前 8 个时从侧栏消失，用户看不出页面被筛选了；搜索在输入或输入法组字时就发出请求；Escape 退出顺序错误，一次清掉了多层状态。前置：读取接口返回多于 8 个标签的概览；当前筛选的标签排在第 8 名之后。触发：从全部标签视图点击该标签；在搜索框中输入（含输入法组字）后按 Enter；依次按 Escape。

必须成立：侧栏显示前 8 个标签并追加当前筛选的标签；筛选行显示 `#标签 · “关键词”` 与接口返回的 `total`；只有 Enter 才发出列表请求，请求同时带 `tag` 与 `q`；再次点击当前标签清除筛选；页面上的 Escape 依次退出放大的卡片、平铺的卡片组、全部标签视图、标签筛选，每次只退一层。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 当前筛选标签始终出现在侧栏，筛选行显示条件与总数 | Covered | `pages/echo.test.tsx::keeps the active tag visible, searches only on Enter and steps back one layer per Escape` |
| 只有 Enter 触发搜索请求，组字过程不发请求 | Covered | `pages/echo.test.tsx::keeps the active tag visible, searches only on Enter and steps back one layer per Escape`（`isComposing` 的 Enter） |
| Escape 每次只退出一层，顺序固定 | Covered | `pages/echo.test.tsx::lets each read recall card leave the pile until the page is opened again`（放大的卡片 → 平铺的卡片组）、`pages/echo.test.tsx::keeps the active tag visible, searches only on Enter and steps back one layer per Escape`（全部标签 → 标签筛选；搜索框内先清空再收起） |

决策依据：D5、D7，不变量 7。

## Composing must extract inline tags and keep the rest of the body intact

风险：`#标签` 识别过宽，Markdown 标题、`C#`、URL 片段被当成标签；提取后压缩了列表缩进和代码块；有内容的草稿因 Escape 或失焦丢失；新札记因当前筛选而不可见。前置：读取接口提供已有标签；当前有标签筛选与搜索。触发：按 `n`，输入包含 `#阅读`、行首 `#rust`、`# 标题`、`C#`、`a.html#x`、缩进列表与代码块的正文；按 Escape、使输入框失焦；Ctrl/Cmd + Enter 提交。

必须成立：上传请求的 `tags` 为 `["阅读", "rust"]`，正文中这两个标签被移除，其余内容（含缩进与代码块）与输入一致，`# 标题`、`C#`、`a.html#x` 保留；有内容时 Escape 与失焦不收起草稿；请求带 UUIDv7 的 `id` 且没有 `title`；提交后筛选与搜索被清除，新卡片带待整理圆点出现在第一列顶部。禁止规范化正文中与标签无关的空白。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 标签提取的边界与正文原样保留 | Covered | `lib/jotting-draft.test.ts::takes out only real tags and keeps every other character as typed`、`pages/echo.test.tsx::writes a jotting with inline tags, keeps a draft through Escape and shows it unfiltered`（上传请求体） |
| 有内容的草稿不因 Escape 或失焦丢失 | Covered | `pages/echo.test.tsx::writes a jotting with inline tags, keeps a draft through Escape and shows it unfiltered` |
| 提交后清除筛选，新卡片可见并带待整理圆点 | Covered | `pages/echo.test.tsx::writes a jotting with inline tags, keeps a draft through Escape and shows it unfiltered`、`e2e/echo.spec.ts::puts a new jotting on top of the first column and moves no other card`（真实 Chromium，三列布局下其他卡片不换列） |

决策依据：D6，不变量 9、10、11。

## Typing right after n must keep the caret in place

风险：输入框所在抽屉用 `overflow: hidden` 裁剪，成为滚动容器；Chromium 在容器长高时错放光标，按 `n` 后立即连续输入的中文变成倒序。前置：真实 Chromium；回响页已加载。触发：按 `n` 后不等待动画结束，立即连续输入一段中文（含输入法提交）与英文。

必须成立：输入框的内容与输入顺序完全一致。这一义务只能以真实 Chromium 为直接证据，jsdom 不实现布局与光标。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 按 `n` 后立即输入的文字顺序不变（真实 Chromium） | Covered | `e2e/echo.spec.ts::keeps text typed right after n in order while the composer slides open`（改回 `overflow: hidden` 时稳定复现倒序） |

决策依据：D6，“风险”中的抽屉裁剪方式。
