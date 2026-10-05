# 时间线加载过渡核心测试用例

当前决策：[先取当天 Moment 的有序轮廓渲染按类型的骨架](../../../decisions/server/moment/20261005-day-outline-and-skeleton-to-card.md)，继承[对话 Moment 卡片字段](../../../decisions/server/moment/20261004-conversation-moment-card-fields.md)。时间线接口本身的区间、排序与卡片字段用例见[时刻身份与时间线](moment-timeline.md)；`moment` 定位参数的既有约定见[文字导航与时刻地址](../../web/navigation/text-nav-and-day-routes.md#returning-from-a-conversation-must-land-on-the-same-day-without-focusing-the-moment)。

## Day outline must list the timeline's moments in the same order

风险：轮廓与时间线的排序、区间或 Owner Scope 不一致，骨架预示的类型与位置和最终卡片对不上，或泄露其他 Owner 的 Moment 身份。前置：一个 Owner 的 Moment 分布在区间边界两侧，其中若干条 `occurred_at` 相同；另一个 Owner 在同一区间也有 Moment。触发：以同一组 `[start, end)` 先后请求轮廓与时间线，并以非法区间（`start >= end`、跨度超过 48 小时、缺少参数）请求两个接口。

必须成立：没有并发写入时，轮廓等于时间线各项 `(id, kind)` 按原顺序组成的序列，包括 `occurred_at` 相同时按 `id` 排序；只包含本 Owner、`start <= occurred_at < end` 的 Moment；两个接口对同一组非法参数返回相同的 400 字段错误；响应带 `Cache-Control: no-store`。禁止轮廓读取详情表才能得到结果，禁止返回区间外或其他 Owner 的 Moment。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 轮廓与时间线的 `(id, kind)` 序列逐项相等，含同时间按 `id` 排序与半开区间边界 | Covered | `moments::day_outline_lists_the_timelines_moments_in_the_same_order`（真实 PostgreSQL）、`authenticated_http_imports_preserve_scope_and_file_parity`（真实 HTTP/PG，响应与时间线逐项投影比较） |
| 轮廓只包含当前 Owner 的 Moment | Covered | `moments::day_outline_lists_the_timelines_moments_in_the_same_order`、`authenticated_http_imports_preserve_scope_and_file_parity` |
| 两个接口对非法区间给出相同的校验结论，轮廓响应带 `no-store` | Covered | `authenticated_http_imports_preserve_scope_and_file_parity`（状态码与响应体逐字节相同）；两者共用同一扫描与校验函数 |

决策依据：D1，不变量 1、2。

## Skeletons must appear only while a day has no cards to show

风险：骨架在已有缓存时闪现，拖慢真实内容；轮廓失败把可以正常显示的日子变成错误页；或轮廓为空时仍长时间留空。前置：时刻页可以分别控制轮廓与时间线两个请求的到达顺序与结果。触发：依次构造“当天已缓存”“轮廓先到且不为空”“轮廓先到且为空”“时间线先到”“轮廓失败”“时间线失败”，并在导入对话后回到当天。

必须成立：当天已缓存时直接显示卡片，不发出轮廓请求；轮廓先到且不为空时按轮廓显示骨架，列表带 `aria-busy="true"`，骨架对辅助技术隐藏；轮廓为空时立即显示“这一天还没有记录。”；时间线先到或轮廓失败时直接显示卡片，不显示骨架、不报错；时间线失败时骨架立即移除并显示错误状态；时间线请求不等待轮廓请求；导入或删除后轮廓缓存随时间线一并失效。禁止为骨架设置最短显示时间，禁止轮廓失败显示错误状态。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 已缓存的日子不请求轮廓、不显示骨架 | Covered | `pages/timeline.test.tsx::never outlines a day whose cards are already cached` |
| 两个请求并行发出，各到达顺序下的呈现符合 D2 | Covered | `pages/timeline.test.tsx::asks for both at once, outlines the day, then turns each placeholder into its own card`、`shows the empty note as soon as the outline is empty, and still shows cards that arrive`、`shows cards without placeholders when the timeline wins or the outline fails` |
| 轮廓失败不报错，时间线失败移除骨架并显示错误 | Covered | `pages/timeline.test.tsx::shows cards without placeholders when the timeline wins or the outline fails`、`drops the placeholders for the error when the timeline fails` |
| 加载期间 `aria-busy` 与骨架的无障碍隐藏 | Covered | `pages/timeline.test.tsx::asks for both at once, outlines the day, then turns each placeholder into its own card` |
| 导入或删除后轮廓缓存随时间线一并失效 | Covered | `app.test.tsx::imports from the day menu at the viewed day and opens the reading page dated to it`、`pages/conversation.test.tsx::deleting a conversation invalidates the outlines of the days it was on` |

决策依据：D2、D5，不变量 4、7。

## Each skeleton must become the card with its own id

风险：骨架按位置错配成别的卡片，并发写入后列表残留加载中的骨架、丢失卡片或顺序错误；过渡结束后内联样式残留，卡片高度被锁死；减少动态效果的设置被忽略；新增 Moment 类型没有骨架。前置：轮廓先于时间线到达。触发：时间线与轮廓完全一致；时间线删掉轮廓中的一项、新增一项、调换两项顺序；在 `prefers-reduced-motion: reduce` 下重复一致的情形。

必须成立：每个骨架所在的列表项以 Moment `id` 为键，原位过渡为同 `id` 的卡片而不重新挂载；时间线中不存在的骨架收拢后移除，轮廓中不存在的卡片在其位置展开，顺序变化的项移动到时间线给出的位置；时间线到达后不再有处于加载状态的骨架；最终显示的卡片及顺序与时间线响应完全一致；过渡结束后列表项没有残留的内联高度、透明度或 `overflow`；过渡期间卡片链接可以点击；减少动态效果时骨架不做起伏、替换不做过渡；每种 `kind` 恰好对应一种骨架形状；对话骨架的高度与标题一行、摘录两行的卡片相同。禁止卡片内容来自轮廓，禁止过渡中拦截点击。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 骨架按 `id` 原位过渡，列表项不重新挂载 | Covered | `pages/timeline.test.tsx::asks for both at once, outlines the day, then turns each placeholder into its own card`、`e2e/timeline-loading.spec.ts::placeholders grow into their own cards, collapse or make room, and leave no trace`（真实 Chromium） |
| 轮廓与时间线不一致时的收拢、展开与移动，最终列表只取时间线 | Covered | `e2e/timeline-loading.spec.ts::placeholders grow into their own cards, collapse or make room, and leave no trace`、`pages/timeline.test.tsx::asks for both at once, outlines the day, then turns each placeholder into its own card` |
| 过渡结束后撤销内联样式，过渡中卡片可点击 | Partial | `e2e/timeline-loading.spec.ts::placeholders grow into their own cards, collapse or make room, and leave no trace`（内联样式与动画撤销）；过渡中的点击没有直接测试 |
| `prefers-reduced-motion: reduce` 下无动画 | Covered | `e2e/timeline-loading.spec.ts::asking for less motion stills the placeholders and swaps them without a morph`（真实 Chromium） |
| 骨架高度与标题一行、摘录两行的卡片完全一致，替换时不跳动 | Covered | `e2e/timeline-loading.spec.ts::a placeholder is exactly as tall as a card with a one-line title and two excerpt lines`（真实 Chromium） |
| 每种 `kind` 恰好一种骨架 | Covered | `DayMoments` 的骨架映射类型为 `Record<MomentKind, …>`，缺少或多出类型时 `task lint:frontend` 的类型检查失败 |

决策依据：D3、D4，不变量 3、5、6、8。

## Scrolling to a moment must wait for card heights to settle

风险：带 `moment` 定位参数冷加载某天时，在骨架过渡中途滚动，高度随后变化，目标停在视口之外。前置：某天的目标 Moment 不在首屏内，当天没有缓存，轮廓先于时间线到达，卡片高度与骨架高度不同。触发：打开 `/?date=…&moment=<id>`。

必须成立：滚动发生在过渡结束、卡片高度稳定之后，目标位于视口中部；随后地址中的 `moment` 被移除；焦点不落在任何 Moment 标题上。没有经过骨架的日子仍在数据到达后立即滚动。禁止在过渡中途滚动。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 经过骨架过渡时，过渡结束后才滚动且目标在视口中部 | Covered | `e2e/timeline-loading.spec.ts::a moment named in the address is centred once the cards stop growing`（真实 Chromium；提前滚动时目标偏离中心约 90px，测试失败） |
| 没有骨架时数据到达后立即滚动，焦点不落在标题上 | Covered | `app.test.tsx::scrolls to the moment named in the address without focusing it, then drops the target` |

决策依据：D5，不变量 9。
