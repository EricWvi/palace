# 网页标注核心测试用例

当前决策：[浏览器插件在原网页上标注，绑定到文章后迁移](../../decisions/extension/0-web-page-annotations-bound-and-migrated-into-articles.md)。文章标注自身的规则见[摘星标注](../web/stars/annotations.md)，本文件只记录原网页标注、绑定与迁移引入的新承诺。数据库约束、级联、事务与并发顺序只能以真实 PostgreSQL 为直接证据。

## Web pages and web annotations must stay within the owner scope

风险：网页或原网页标注接口没有按 Owner 过滤，其他 Owner 能列出、读到、修改或删除别人的原网页标注；能把自己的网页绑定到别人的文章；能通过迁移接口把别人的原网页标注迁进自己的文章，或把自己的迁进别人的文章。前置：真实 Router 与真实 PostgreSQL；Owner A 有一个未绑定网页（带原网页标注）、一个绑定到 A 的文章的网页（带剩余原网页标注）；Owner B 有自己的文章。触发：B 用 A 的网址请求 `lookup`、新建标注；B 对 A 的原网页标注 `PUT`、`DELETE`；B 把自己的网页绑定到 A 的文章；B 请求 A 的文章的 `web-annotations` 与 `migrate`；B 在自己文章的 `migrate` 请求中列出 A 的原网页标注 ID；未登录请求全部接口。

必须成立：B 读不到 A 的任何网页或原网页标注，跨 Owner 的读写都得到与不存在相同的 404；B 用相同网址新建标注只会得到 B 自己的网页；B 的迁移请求中不属于 B 的项被跳过，不新建标注、不删除 A 的行；未登录的请求被拒绝。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 跨 Owner 的读、写、绑定与迁移都不触及别人的行（真实 PostgreSQL） | Covered | `crates/db/tests/postgres/web_pages.rs::web_pages_must_stay_within_the_owner_scope`；迁移请求列出别人的标注、请求别人的文章见 `crates/db/tests/postgres/web_migration.rs::migration_must_atomically_replace_web_annotations` |
| 接口形状、状态码与 OpenAPI 合同一致（真实 Router） | Covered | `crates/backend/tests/http.rs::authenticated_http_imports_preserve_scope_and_file_parity`（经 `tests/http/web_pages.rs::exercise_web_pages`，响应由 OpenAPI 校验器逐一核对；未登录请求由同一测试对业务路由层的检查覆盖） |

决策依据：D9，不变量 1、2。

## A bound web page must reject new and changed web annotations

风险：绑定后原网页仍能新增或修改原网页标注，同一处标注在原网页与 Palace 文章上各有一份可修改的副本；或者网页状态之外的字段组合出现非法状态，例如未绑定却没有任何标注的空网页留在列表里。前置：真实 Router 与真实 PostgreSQL；一个未绑定网页带两处原网页标注。触发：删除其中一处，再删除最后一处；重新新建一处后绑定到一篇文章；绑定后对该网址新建标注、对剩余标注 `PUT`、`DELETE`；改绑到另一篇文章；对一个尚无记录的网址直接绑定。

必须成立：删除最后一处时网页记录在同一事务中删除；绑定后新建返回 409、修改返回 409，删除成功；改绑只改变 `article_id`；对无记录网址绑定会创建已绑定的网页；网页标题在新建、修改与绑定时更新。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 已绑定网页拒绝新建与修改，允许删除（真实 PostgreSQL） | Covered | `crates/db/tests/postgres/web_pages.rs::a_bound_web_page_must_reject_new_and_changed_web_annotations`；409 `web_page_bound` 的接口形状见 `exercise_web_pages` |
| 未绑定网页随最后一处标注一起删除；绑定与改绑的状态转换（真实 PostgreSQL） | Covered | `crates/db/tests/postgres/web_pages.rs::a_bound_web_page_must_reject_new_and_changed_web_annotations`；表上的提交时检查拒绝没有标注的未绑定网页、触发器拒绝改锚点，见 `web_page_rows_must_keep_annotations_and_anchors` |
| 字段校验：网址、标题、引文、前后文长度、`position` 范围、样式与批注至少一个（领域单元测试） | Covered | `crates/domain/src/web_page.rs` 的 `addresses_are_absolute_without_fragment`、`titles_fall_back_to_the_address_and_fit_the_limit`、`anchors_check_quote_context_and_position`；样式与批注沿用 `crates/domain/src/annotation.rs::content_needs_a_style_or_a_note` |

决策依据：D2、D3、D9，不变量 3、8。

## Migrating a web annotation must atomically replace it with exactly one article annotation

风险：迁移只完成一半，原网页标注被删除但文章标注没有建成，读者的批注丢失；或文章标注建成但原网页标注还在，下次打开文章页又迁移一次，产生重复标注；两个标签页同时迁移同一批标注，各新建一份；新标注的样式、批注或创建时间没有取自原网页标注。前置：真实 PostgreSQL；一篇文章，绑定到它的网页上有三处原网页标注。触发：一次迁移其中两处；对第三处提交一个 `quote` 长度与 `[start, end)` 不符的锚点；用 barrier 让两个事务在删除步骤前对齐，同时迁移同一批标注；迁移一个属于未绑定网页或绑定到另一篇文章的原网页标注。

必须成立：成功的迁移删除被列出的原网页标注，并为每一处恰好新建一处文章标注，样式、批注与 `created_at` 取自原网页标注；校验失败时整个事务回滚，原网页标注全部保留；并发迁移结束后每处原网页标注只对应一处文章标注，后到的事务跳过已删除的项而不报错；不属于这篇文章的项被跳过。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 删除与新建在一个事务中提交或回滚（真实 PostgreSQL，注入校验失败） | Covered | `crates/db/tests/postgres/web_migration.rs::migration_must_atomically_replace_web_annotations`（一个违反 `annotation` 检查约束的锚点让整条迁移语句失败，三处原网页标注全部保留） |
| 并发迁移同一批标注收敛为每处一份（真实 PostgreSQL，barrier 对齐） | Covered | `crates/db/tests/postgres/web_migration.rs::concurrent_migrations_create_one_annotation_each`（第三个事务锁住这批行，等两个迁移都排队在删除这一步上再放行） |
| 新标注继承样式、批注与创建时间，并满足标注锚点约束（真实 PostgreSQL） | Covered | `crates/db/tests/postgres/web_migration.rs::migration_must_atomically_replace_web_annotations`；`quote` 长度不符的整个请求返回 400 见 `exercise_web_pages` |

决策依据：D5，不变量 4、5。

## Relocation must anchor a web annotation on the same words in the rendered article

风险：重新定位把原网页标注画到 Palace 正文里相邻或相似的文字上；块之间的空白差异让引文找不到；同一句话出现多次时选错一处；换算回原始下标时差一位，新标注的 `quote` 与 `[start, end)` 不一致；找不到时仍然勉强给出一个位置。前置：前端单元测试（纯函数，表驱动）与真实 Chromium。触发：原网页引文跨越段落、列表项与行内代码，夹带换行与缩进；同一引文在正文中出现多次，前后文只与其中一处吻合，或前后文都不吻合、只能靠 `position` 区分；引文在正文中不存在；正文含有标注文字不计入的复制按钮与图片占位。

必须成立：定位结果的 `quote` 等于标注文字中 `[start, end)` 那一段，首尾不是空白；多处出现时选中前后文吻合最长的一处，分数相同时选 `position` 最近的一处；找不到时报告定位失败，不产生锚点；迁移后标注画在与原网页相同的文字上，失败的条目出现在「没对上」的列表里并可以丢弃。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 空白规范化、候选打分与下标换算（前端单元测试） | Covered | `apps/palace-web/src/lib/text-quote.test.ts`；插件一侧选区到前后文、再到 DOM 范围的换算见 `apps/palace-extension/src/page-text.test.ts` |
| 从原网页标注到文章页标记的完整迁移流程，以及未定位条目的显示与丢弃（真实 Chromium，接口直接写入原网页标注） | Covered | `apps/palace-web/e2e/web-annotations.spec.ts` 的 “web annotations migrate onto the same words, and the unmatched one can be discarded”（接口由 `page.route` 按服务端规则模拟；服务端一侧见上一个用例） |

决策依据：D5，不变量 5、6。

## Deleting an article must take its bound web pages with it

风险：删除文章后，绑定到它的网页仍然指向不存在的文章，插件在原网页上请求文章标注时出错；剩余的原网页标注成为无法迁移、也无法在 Palace 里丢弃的孤儿；级联删除与文章删除不在同一事务中。前置：真实 PostgreSQL；一篇文章绑定了两个网页，其中一个还有剩余的原网页标注；另有一个未绑定网页。触发：删除这篇文章；让文章删除的事务在提交前失败。

必须成立：删除成功后两个绑定网页及其剩余标注都不存在，未绑定网页不受影响；删除失败时文章、网页与原网页标注全部保留。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 文章删除级联删除绑定网页与剩余原网页标注，失败时全部保留（真实 PostgreSQL） | Covered | `crates/db/tests/postgres/web_pages.rs::deleting_an_article_must_take_its_bound_web_pages_with_it`（提交时触发器注入失败） |

决策依据：D3、D9，不变量 7。

## Candidate articles must rank the nearest address first

风险：候选列表把同域名的文章排在后面，读者找不到正确的文章，绑定到错误的文章后迁移不可撤销；带 `near` 的分页游标与排序键不一致，翻页时重复或漏掉文章；`near` 与 `q` 同时使用时排序或过滤失效。前置：真实 PostgreSQL；若干文章分布在同一主机名（含 `www.` 前缀与大小写差异）的不同路径深度、其他主机名上，导入时间各不相同。触发：带 `near` 分页读取全部候选；带 `near` 与 `q` 读取。

必须成立：同主机名的文章排在前面，其中公共路径段越多越靠前，其余按导入时间倒序；逐页读取的结果与一次读取的顺序相同，无重复、无遗漏；`q` 只过滤，不改变上述排序；`total` 不受 `q` 影响。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| `near` 排序、分页游标与 `q` 组合（真实 PostgreSQL） | Covered | `crates/db/tests/postgres/web_migration.rs::candidate_articles_must_rank_the_nearest_address_first`；排序规则本身见 `crates/domain/src/web_page/nearness.rs::nearness_counts_host_then_shared_segments` |

决策依据：D4。

## The access token must never reach the page context

风险：插件把 Access Token 交给内容脚本或写进网页可读的位置，网页上的脚本读到 token 后可以调用 Palace 全部业务接口；token 经 `chrome.storage.sync` 传播到其他设备。前置：真实 Chromium 加载插件，打开一个会读取全局变量、DOM 与消息的测试网页。触发：配置 token 后在测试网页上标注、绑定并显示只读标注；检查网页上下文可见的全局对象、DOM、`postMessage` 记录与 `chrome.storage.sync`。

必须成立：所有 API 请求都由 service worker 发出；网页上下文中任何位置都找不到 token；`chrome.storage.sync` 中没有 token；token 无效时插件停止显示小条并提示。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| token 只存在于 service worker 与 `chrome.storage.local`（真实 Chromium） | Covered | `apps/palace-extension/e2e/extension.spec.ts` 的 “annotating, binding and the read-only display never put the token in the page”、“an invalid token turns the bar off and marks the toolbar icon”（加载构建后的插件，Palace 由测试内的 HTTP 服务模拟）；内容脚本只能发出本页的四种请求、设置不回传 token 见 `apps/palace-extension/src/palace.test.ts` |

决策依据：D8，不变量 9。
