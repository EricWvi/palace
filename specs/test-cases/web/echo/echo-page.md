# 回响页核心测试用例

当前决策：[回响页](../../../decisions/web/echo/0-jotting-wall-with-daily-recall-pile.md)。服务端读取接口的承诺见[札记读取接口](jotting-reading.md)；本文件只记录页面行为。

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

必须成立：收起卡片组的点击不改变筛选；每关闭一张，卡片组少一张，其余顺序不变；全部读完后显示“今天的回响都已看过”；重新挂载后卡片组恢复为接口返回的 5 张；有筛选或搜索时卡片组收起，清除后恢复，已读的卡片不回来。禁止把已读状态写入任何持久存储。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 关闭即离开卡片组，读完显示提示，重新挂载后恢复 | Covered | `pages/echo.test.tsx::lets each read recall card leave the pile until the page is opened again`（同时断言没有写入 `localStorage`） |
| 收起卡片组的点击被拦截，不触发瀑布流操作 | Covered | `pages/echo.test.tsx::lets each read recall card leave the pile until the page is opened again` |
| 筛选与搜索时卡片组收起，清除后恢复 | Covered | `pages/echo.test.tsx::lets each read recall card leave the pile until the page is opened again`（标签筛选与搜索使卡片组所在抽屉变为 inert，清除后恢复） |

决策依据：D3，不变量 3、8。

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
