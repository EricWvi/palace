# 回响页核心测试用例

当前决策：[回响页](../../../decisions/web/echo/0-jotting-wall-with-daily-recall-pile.md)、[瀑布流的札记点时间放大](../../../decisions/web/echo/20261101-zoom-wall-jottings-by-their-time-and-load-palace-media.md)、[回顾按完成次数最少者优先抽取](../../../decisions/web/echo/20261101-recall-least-completed-first-from-a-stored-daily-pile.md)、[放大卡片里编辑和删除札记](../../../decisions/web/echo/20261101-edit-and-delete-a-zoomed-jotting-with-title-slips.md)、[带空格的标签写成 `#[[词 词]]`](../../../decisions/web/echo/20261101-wrap-tags-with-spaces-in-double-brackets.md)、[札记正文用 `==文字==` 高亮](../../../decisions/web/echo/20261113-highlight-text-with-double-equals-in-jottings.md)。服务端读取接口的承诺见[札记读取接口](jotting-reading.md)；本文件只记录页面行为。

## Echo navigation must open the wide jotting page

风险：回响仍不可点击，或点击后进入错误的地址；回响页没有加宽，或加宽泄漏到其他页面；导航当前项错乱。前置：已登录，读取接口返回若干札记。触发：点击顶部导航的「回响」；从回响点击「时刻」；直接打开 `/jottings`。

必须成立：「回响」是指向 `/jottings` 的链接，回响页上只有回响带 `aria-current="page"`；回响页的页面容器最大宽度约 1200px，离开后恢复为 760px；时刻、摘星的行为不变。禁止加宽样式作用于其他页面。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 回响指向 `/jottings`，只有回响是当前项 | Covered | `app.test.tsx::marks only 时刻, keeps other sections out of reach, and links cards to the reading page`、`pages/echo.test.tsx::widens the sheet for 回响 only and marks 回响 as the current section` |
| 只有回响页加宽，离开后恢复 | Partial | `pages/echo.test.tsx::widens the sheet for 回响 only and marks 回响 as the current section`（页面容器的 `wide` 类）；实际宽度由 CSS 决定，jsdom 不计算布局 |

决策依据：D1，Web 导航根决策 D1 的修改项。

## Only completing a recall card may take it out of the pile

风险：直接关闭放大的回顾卡片也让它离开卡片组；点「温故」后请求失败，卡片却已离开；刷新后完成过的卡片又回来；收起平铺卡片组的点击同时触发了瀑布流中的标签或“展开”；筛选时卡片组仍然占据顶部。前置：回顾接口返回若干张卡片，其中一张 `done: true`；瀑布流中有可点击的标签。触发：点击卡片组平铺；点击瀑布流上的标签位置收起；再次平铺、放大一张后按 Escape；放大另一张点「温故」；放大一张点「束阁」；让一次完成请求失败；完成全部卡片；重新挂载页面；按标签筛选后再清除。

必须成立：`done: true` 的卡片不显示；Escape 或点背景关闭时卡片缩回并留在卡片组中，不发出完成请求；「温故」发出 `next: "keep"`、「束阁」发出 `next: "retire"`，成功后卡片离开卡片组；请求失败时卡片保持放大，按钮可以再点；全部完成后显示“今天的回响都已看过”；重新挂载后完成过的卡片不回来；收起卡片组的点击不改变筛选；有筛选或搜索时卡片组收起，清除后恢复。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 直接关闭不离开、不请求；完成和束阁发出对应请求后离开 | Covered | `pages/echo.test.tsx::takes a recall card out of the pile only when it is completed, and remembers it` |
| 完成失败时卡片保持放大 | Covered | `pages/echo.test.tsx::takes a recall card out of the pile only when it is completed, and remembers it` |
| 全部完成显示提示，刷新后完成的不回来 | Covered | `pages/echo.test.tsx::takes a recall card out of the pile only when it is completed, and remembers it`（服务端替身在两次访问之间保留完成状态） |
| 收起卡片组的点击被拦截，不触发瀑布流操作 | Covered | `pages/echo.test.tsx::takes a recall card out of the pile only when it is completed, and remembers it` |
| 筛选与搜索时卡片组收起，清除后恢复 | Covered | `pages/echo.test.tsx::takes a recall card out of the pile only when it is completed, and remembers it`（标签筛选与搜索使卡片组所在抽屉变为 inert，清除后恢复） |
| 完成的卡片缩回原位并淡出，被移除前不会回到屏幕中央闪现（真实 Chromium） | Covered | `e2e/echo.spec.ts::a completed recall card fades out at its slot and never flashes back at full size`（逐帧记录不透明度） |

决策依据：根决策 D3，20261101 回顾决策 D5、D7，根决策不变量 8。

## A zoomed jotting must say whether it takes part in recall

风险：瀑布流的放大卡片出现「温故」；还在 inbox 的札记出现「束阁」或「启封」，或者按 `pending` 判断，让网页改过的已整理札记也失去它们；「束阁」与「启封」的文字与札记状态不符；切换后卡片被关闭；操作随卡片缩回而“飞”回原位。前置：瀑布流中一张参与回顾、一张束阁的札记。触发：分别放大两张，点击右侧的操作；关闭放大卡片。

必须成立：瀑布流放大卡片只显示「束阁」或「启封」，与 `recall` 一致，`recall` 为 `inbox` 时两者都不显示；点击后发出 `PUT /api/jottings/{id}/recall`，文字切换，卡片保持放大；回顾卡片放大后显示「温故」和「束阁」；关闭开始时操作立即淡出。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 两种放大卡片的操作与请求 | Covered | `pages/echo.test.tsx::takes a recall card out of the pile only when it is completed, and remembers it`（回顾卡片的两个操作）、`pages/echo.test.tsx::reads a wall jotting in full from its time, with its Palace pictures, and hands it back`（瀑布流的束阁与启封）；操作在关闭开始时淡出由 CSS 决定，jsdom 不计算 |
| inbox 札记没有回顾题签，待整理但已归位的札记仍有 | Covered | `pages/echo.test.tsx::offers no recall slip for a jotting still in the inbox, only for a sorted one` |
| 题签按编辑决策 D1 的顺序排列：瀑布流「修订 / 束阁 / 削简」，回顾「温故 / 束阁 / 修订 / 削简」 | Covered | `pages/echo.test.tsx::takes a recall card out of the pile only when it is completed, and remembers it`、`pages/echo.test.tsx::reads a wall jotting in full from its time, with its Palace pictures, and hands it back` |

决策依据：20261101 回顾决策 D5、D6；20261101 编辑决策 D1；[inbox 札记不参与回顾](../../../decisions/web/echo/20261107-inbox-jottings-take-no-part-in-recall.md)。

## The spread pile must lay out up to three rows of six

风险：多于 6 张时卡片仍排成一行，伸出页面右侧；窄一些的视口下没有换行；补位时卡片跨行斜着长距离移动。前置：真实 Chromium；1280px 宽的视口；卡片组 18 张。触发：平铺；完成第一行的一张；把视口缩到放不下 6 列再平铺。

必须成立：18 张排成 3 行、每行 6 张，全部在视口宽度以内；完成一张后其余卡片按顺序前移一格；视口放不下 6 列时按能放下的列数换行。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 多行平铺的位置与换行（真实 Chromium） | Covered | `e2e/echo.spec.ts::a pile of eighteen spreads into three rows of six and closes ranks as cards leave`（18 张排成 3×6，完成一张后按序前移，900px 宽时少于 6 列） |

决策依据：20261101 回顾决策 D6。

## A wall jotting must open in full by its time and come back as it was

风险：点击正文或空白也会放大，选字、点链接时误开；超长札记在墙上原地展开，拉长所在的列；放大期间的 Escape 同时清掉了标签筛选；关闭后原卡片不见了，或展开状态被改变。前置：瀑布流中有一张近期札记、一张展开后约 20 行的札记和一张超过 40 行的札记；当前按标签筛选。触发：点击卡片正文；点击页脚的时间；按 Escape；点击两张长札记的「展开」。

必须成立：点击正文不放大，点击时间才放大；放大期间原卡片隐藏、下面的页面不能滚动、瀑布流宽度不变，关闭后重新显示在原位、页面恢复滚动；Escape 只关闭放大卡片，标签筛选不变；不超过 40 行的札记原地展开，超过 40 行的札记点「展开」直接放大，墙上的卡片保持折叠。禁止瀑布流卡片的时间带悬停提示；禁止放大卡片显示相对时间。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 只有时间是放大入口，放大卡片显示完整时间，瀑布流时间没有提示 | Covered | `pages/echo.test.tsx::reads a wall jotting in full from its time, with its Palace pictures, and hands it back` |
| Escape 只关闭放大卡片，原卡片回到墙上 | Covered | `pages/echo.test.tsx::reads a wall jotting in full from its time, with its Palace pictures, and hands it back` |
| 放大期间页面不随滚轮或翻页键滚动，瀑布流宽度不变，关闭后恢复滚动（真实 Chromium） | Covered | `e2e/echo.spec.ts::the page under a zoomed jotting does not scroll, and the wall keeps its width`（去掉锁定时稳定失败） |
| 40 行以内原地展开，超过 40 行展开即放大 | Partial | `pages/echo.test.tsx::opens a body in place up to forty lines and reads a longer one in full`（jsdom 没有布局，正文高度与行高由测试替换） |
| 回顾卡片直接关闭时留在卡片组，完成后才离开 | Covered | `pages/echo.test.tsx::takes a recall card out of the pile only when it is completed, and remembers it`，见 [Only completing a recall card may take it out of the pile](#only-completing-a-recall-card-may-take-it-out-of-the-pile) |

决策依据：20261101 决策 D1–D3、D5，不变量 1–3、7。

## Only a jotting read in full may load pictures, and only Palace media

风险：缩略卡片请求图片，图片晚到后打乱瀑布流各列；打开旧札记时浏览器请求了外站图片；手机截图按卡片宽度显示，高过一屏；卡片按打开时测得的高度固定，图片加载后只能在框内滚动。前置：札记正文同时引用 `/api/media/<uuid>` 与外站图片；媒体响应晚于放大动画结束。触发：在瀑布流中查看卡片；点击时间放大；等待图片到达。

必须成立：瀑布流卡片中两张图片都显示占位，不请求图片；放大卡片只加载 `/api/media/<uuid>`，外站地址显示占位；图片显示高度不超过 360px；图片到达后放大卡片变高，不超过视口的 80%，并保持垂直居中。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 缩略卡片只显示占位，放大卡片只加载 Palace 媒体 | Covered | `pages/echo.test.tsx::reads a wall jotting in full from its time, with its Palace pictures, and hands it back` |
| 图片限高 360px，卡片随图片居中长高且不超过视口 80%（真实 Chromium） | Covered | `e2e/echo.spec.ts::a jotting read in full grows evenly for a late tall picture and caps its height`（媒体响应被扣住到卡片放大完毕之后）、`e2e/echo.spec.ts::a jotting longer than the screen never shows past its cap while it opens`（逐帧记录高度，放大结束的那一帧也不超过 80%） |

决策依据：20261101 决策 D4、D5，不变量 4–6。

## Editing a zoomed jotting must never lose the draft

风险：标签没有写回正文，保存时被清空；带空格的标签写回后被截断；保存后的标签提取与新建不一致；有改动时误按 Escape 或点背景丢掉草稿；保存冲突时草稿被最新内容替换；编辑器超出放大卡片的最大高度。前置：瀑布流和回顾卡片组各有一张带标题和标签（含带空格的标签）的札记，服务端替身可以返回 `409` 与其他错误。触发：放大后点「修订」；不改动按 Escape；改动后按 Escape、点背景；Ctrl/Cmd+Enter 保存；让一次保存返回 `409 jotting_changed`，另一次返回 `500`；在真实 Chromium 中输入超过一屏的正文。

必须成立：编辑器中标题在单行输入框，正文末尾隔一空行写着 `#标签`，带空格的写成 `#[[标签]]`，光标在它们之前；无改动时 Escape 回到阅读状态、「保存」不发请求；有改动时 Escape 和点背景不起作用；保存请求带打开时的 `version`，按新建规则提取的标签、去掉标签的正文和去掉空白的标题；成功后回到阅读状态，墙上和卡片组的卡片显示新内容并带待整理圆点；`409` 时保留草稿、显示说明、「保存」不可点；其他错误保留草稿、可以重试；编辑器随内容长高，超过最大高度时在卡片内滚动。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 标签写回正文、保存时按新建规则提取，请求带 `version` | Covered | `pages/echo.test.tsx::edits a zoomed jotting in place with its tags written back, and never drops a changed draft` |
| 带空格的标签写回为 `#[[…]]`，保存后原样不变 | Covered | `pages/echo.test.tsx::edits a zoomed jotting in place with its tags written back, and never drops a changed draft`（`Vibe Coding`）、`lib/jotting-draft.test.ts::reads back every tag it writes` |
| 有改动时 Escape 和点背景不丢草稿；无改动时 Escape 逐层退出 | Covered | `pages/echo.test.tsx::edits a zoomed jotting in place with its tags written back, and never drops a changed draft` |
| 保存成功后墙和卡片组同步更新并带圆点 | Partial | `pages/echo.test.tsx::edits a zoomed jotting in place with its tags written back, and never drops a changed draft`（瀑布流的卡片与放大卡片）；卡片组一侧只由同一个 `patchJotting` 写缓存，没有单独断言 |
| `409` 与其他错误都保留草稿，`409` 时不可再保存 | Covered | `pages/echo.test.tsx::edits a zoomed jotting in place with its tags written back, and never drops a changed draft` |
| 编辑器不超过放大卡片的最大高度（真实 Chromium） | Covered | `e2e/echo.spec.ts::a deleted jotting leaves its column alone after a vermilion 确认, and an edit stays within its cap` |

决策依据：20261101 编辑决策 D2、D3，不变量 2、4；`#[[…]]` 决策 D1，不变量 1。

## Deleting a jotting must take two clicks on the same slip

风险：一次点击就删除；「确认」作废规则失效，隔很久的第二次点击仍然删除；题签变长挤动旁边的题签；删除失败时卡片已经离开；删除后卡片留在墙上或卡片组中。前置：瀑布流和回顾卡片组各有一张札记，服务端替身可以让删除失败；可控的计时器。触发：放大后点「削简」后等 3 秒；再点「削简」，2 秒后移开鼠标，再等 1 秒；点「削简」后让题签失焦；点「削简」再点「确认」；让一次删除返回 `409`。

必须成立：第一次点击只把题签变成「确认」，不发请求；第一次点击后 3 秒或失焦时变回「削简」，移开鼠标既不作废也不重新计时；在「确认」上再点才发出带 `version` 的删除请求；成功后放大卡片缩回空位，回顾卡片离开卡片组，瀑布流卡片从所在列移除、其他列不动；失败时放大卡片保持打开、显示说明。「确认」的颜色与题签宽度由 CSS 决定，以真实 Chromium 为直接证据。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 两次点击才删除，超时或失焦作废第一次，移开鼠标不影响计时 | Covered | `pages/echo.test.tsx::deletes a jotting only on a second click on the same slip, from the wall and from the pile`（超时用假计时器） |
| 删除后离开卡片组或所在列，同列下面的卡片滑上来补位，其他列不动 | Covered | `pages/echo.test.tsx::deletes a jotting only on a second click on the same slip, from the wall and from the pile`（离开墙与卡片组）、`e2e/echo.spec.ts::a deleted jotting leaves its column alone after a vermilion 确认, and an edit stays within its cap`（真实 Chromium，三列布局下其他卡片不换列，下面一张卡片逐帧经过中间位置；去掉过渡时稳定失败） |
| 删除失败时卡片保持打开 | Covered | `pages/echo.test.tsx::deletes a jotting only on a second click on the same slip, from the wall and from the pile` |
| 「确认」为朱红色且题签宽度不变，删除后缩回空位（真实 Chromium） | Partial | `e2e/echo.spec.ts::a deleted jotting leaves its column alone after a vermilion 确认, and an edit stays within its cap`（颜色与宽度）；缩回空位的动画沿用「温故」的 `landing="empty"`，没有逐帧检查 |

决策依据：20261101 编辑决策 D1、D4，不变量 1、3、4。

## Tag filters and search must stay visible and intersect

风险：当前筛选的标签不在前 8 个时从侧栏消失，用户看不出页面被筛选了；搜索在输入或输入法组字时就发出请求；Escape 退出顺序错误，一次清掉了多层状态。前置：读取接口返回多于 8 个标签的概览；当前筛选的标签排在第 8 名之后。触发：从全部标签视图点击该标签；在搜索框中输入（含输入法组字）后按 Enter；依次按 Escape。

必须成立：侧栏显示前 8 个标签并追加当前筛选的标签；筛选行显示 `#标签 · “关键词”` 与接口返回的 `total`；只有 Enter 才发出列表请求，请求同时带 `tag` 与 `q`；再次点击当前标签清除筛选；页面上的 Escape 依次退出放大的卡片、平铺的卡片组、全部标签视图、标签筛选，每次只退一层。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 当前筛选标签始终出现在侧栏，筛选行显示条件与总数 | Covered | `pages/echo.test.tsx::keeps the active tag visible, searches only on Enter and steps back one layer per Escape` |
| 只有 Enter 触发搜索请求，组字过程不发请求 | Covered | `pages/echo.test.tsx::keeps the active tag visible, searches only on Enter and steps back one layer per Escape`（`isComposing` 的 Enter） |
| Escape 每次只退出一层，顺序固定 | Covered | `pages/echo.test.tsx::takes a recall card out of the pile only when it is completed, and remembers it`（放大的卡片 → 平铺的卡片组）、`pages/echo.test.tsx::keeps the active tag visible, searches only on Enter and steps back one layer per Escape`（全部标签 → 标签筛选；搜索框内先清空再收起） |

决策依据：D5、D7，不变量 7。

## Composing must extract inline tags and keep the rest of the body intact

风险：`#标签` 识别过宽，Markdown 标题、`C#`、URL 片段被当成标签；选中带空格的已有标签后被截断；提取后压缩了列表缩进和代码块；有内容的草稿因 Escape 或失焦丢失；新札记因当前筛选而不可见。前置：读取接口提供已有标签；当前有标签筛选与搜索。触发：按 `n`，输入包含 `#阅读`、行首 `#rust`、`# 标题`、`C#`、`a.html#x`、缩进列表与代码块的正文；按 Escape、使输入框失焦；Ctrl/Cmd + Enter 提交。

必须成立：上传请求的 `tags` 为 `["阅读", "rust"]`，正文中这两个标签被移除，其余内容（含缩进与代码块）与输入一致，`# 标题`、`C#`、`a.html#x` 保留；`#[[Vibe Coding]]` 提取为 `Vibe Coding`，补全带空格的标签时写成这种形式，同一行后面的 `C#` 不改变标签范围；有内容时 Escape 与失焦不收起草稿；请求带 UUIDv7 的 `id` 且没有 `title`；提交后筛选与搜索被清除，新卡片带待整理圆点出现在第一列顶部；第一列先下滑让位、新卡片再淡入，不是一帧跳到位。禁止规范化正文中与标签无关的空白。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 带空格的标签：`#[[…]]` 提取、补全写法、不受 `C#` 影响 | Covered | `lib/jotting-draft.test.ts::reads a tag with spaces wrapped in [[ ]]`、`lib/jotting-draft.test.ts::finds the fragment before the caret and completes it with a space` |
| 标签提取的边界与正文原样保留 | Covered | `lib/jotting-draft.test.ts::takes out only real tags and keeps every other character as typed`、`pages/echo.test.tsx::writes a jotting with inline tags, keeps a draft through Escape and shows it unfiltered`（上传请求体） |
| 有内容的草稿不因 Escape 或失焦丢失 | Covered | `pages/echo.test.tsx::writes a jotting with inline tags, keeps a draft through Escape and shows it unfiltered` |
| 提交后清除筛选，新卡片可见并带待整理圆点 | Covered | `pages/echo.test.tsx::writes a jotting with inline tags, keeps a draft through Escape and shows it unfiltered`、`e2e/echo.spec.ts::puts a new jotting on top of the first column and moves no other card`（真实 Chromium，三列布局下其他卡片不换列） |
| 第一列下滑让位、新卡片随后淡入（真实 Chromium） | Covered | `e2e/echo.spec.ts::puts a new jotting on top of the first column and moves no other card`（逐帧记录原顶部卡片的位置和新卡片的不透明度；去掉过渡时稳定失败） |

决策依据：D6，不变量 9、10、11；`#[[…]]` 决策 D1，不变量 1、2。

## Typing right after n must keep the caret in place

风险：输入框所在抽屉用 `overflow: hidden` 裁剪，成为滚动容器；Chromium 在容器长高时错放光标，按 `n` 后立即连续输入的中文变成倒序。前置：真实 Chromium；回响页已加载。触发：按 `n` 后不等待动画结束，立即连续输入一段中文（含输入法提交）与英文。

必须成立：输入框的内容与输入顺序完全一致。这一义务只能以真实 Chromium 为直接证据，jsdom 不实现布局与光标。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 按 `n` 后立即输入的文字顺序不变（真实 Chromium） | Covered | `e2e/echo.spec.ts::keeps text typed right after n in order while the composer slides open`（改回 `overflow: hidden` 时稳定复现倒序） |

决策依据：D6，“风险”中的抽屉裁剪方式。

## Jotting bodies must highlight double equals and nothing else may

风险：`==` 在所有页面生效，导入对话里的 `a==b and c==d` 被标成高亮；`a == b`、`===` 或转义的 `\==` 被误认成记号；全角标点挨着记号时配不成对；高亮内部的加粗、斜体没有解析；没配上的 `==` 连同文字一起丢失。前置：札记方言与默认方言分别渲染同一批正文。触发：渲染含成对记号、全角标点、嵌套格式、软换行、空白两侧、三个等号、转义、行内代码和未闭合记号的正文。

必须成立：札记方言下成对的 `==文字==` 渲染为 `<mark>`，内部格式照常解析；其余情形原样显示；默认方言下 `==` 原样显示；瀑布流卡片、放大卡片和卡片组都用札记方言。禁止 `==` 在默认方言下生效，禁止未配对的 `==` 或其中的文字消失。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 成对记号渲染为 `<mark>`，含全角标点、嵌套格式与软换行 | Covered | `lib/remark-highlight-mark.test.tsx::marks %j` |
| 空白两侧、`=` 与 `===`、转义、行内代码、未闭合的 `==` 原样显示 | Covered | `lib/remark-highlight-mark.test.tsx::leaves %j as text` |
| 默认方言不认 `==` | Covered | `lib/remark-highlight-mark.test.tsx::keeps \`==\` as text outside jottings` |
| 瀑布流卡片与放大卡片使用札记方言 | Covered | `pages/echo.test.tsx::highlights ==text== in a jotting on the wall and when zoomed` |
| 卡片组的卡片使用札记方言 | Missing | 卡片组与瀑布流共用同一套正文渲染，但没有测试直接渲染卡片组里的高亮 |

决策依据：`==` 高亮决策 D1、D2，不变量 1、2、3。
