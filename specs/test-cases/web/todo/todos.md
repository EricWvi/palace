# 行事核心测试用例

当前决策：[行事页是「今日」加集合的目录式页面](../../../decisions/web/todo/0-today-and-lists-with-fractional-order.md)、[集合按拖动的顺序排列](../../../decisions/web/todo/20261031-lists-in-dragged-order.md)。数据库约束、唯一索引与级联只能以真实 PostgreSQL 为直接证据；拖动排序以真实 Chromium 为直接证据。

## Tasks and lists must stay within the owner scope

风险：任务或集合接口没有按 Owner 过滤，其他 Owner 能看到、修改、完成、删除别人的任务或集合；或者能把任务放进别人的集合；或者把别人的任务 id 塞进「安排今日」，改掉别人的排期。前置：真实 Router 与真实 PostgreSQL；Owner A 有一个集合、其中的任务和一条收件箱任务，Owner B 有自己的集合。触发：B 对 A 的任务请求编辑、正文读写、今日已做、完成、删除，对 A 的集合请求改名、已完成列表、清空、删除，向 A 的集合新建任务；A 把自己的任务移进 B 的集合；B 用 A 的任务 id 安排今日；B 读取看板。

必须成立：每个请求都得到与不存在相同的 404，A 的看板不变；B 的「安排今日」不改变任何行；B 的看板为空。

禁止结果：跨 Owner 的请求返回 403、409 等能证明对方存在的状态，或改变任何一行。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 跨 Owner 的读写都是 404 且不改变数据，安排今日只作用于自己的任务（真实 PostgreSQL） | Covered | `crates/db/tests/postgres/todos.rs::todos_must_stay_within_the_owner_scope` |
| 接口形状、状态码与 OpenAPI 合同一致（真实 Router） | Covered | `crates/backend/tests/http.rs::authenticated_http_imports_preserve_scope_and_file_parity`（经 `tests/http/todos.rs::exercise_todos`，响应由 OpenAPI 校验器逐一核对） |

决策依据：D5，不变量 1。

## A task must keep one plan and the inbox none

风险：搁置与某一天同时存在，页面不知道该显示哪个；收件箱里的任务被排期或完成，绕过了「先整理再安排」；改了排期却保留着今日已做，昨天的勾带到了今天；已完成的任务仍挂着排期，恢复后突然出现在今日；过去的日子被当成真实计划，任务永远离不开某一天。前置：领域单元测试、真实 PostgreSQL、组件测试。触发：给收件箱任务提交搁置或某一天；完成收件箱任务；直接在数据库里写出上述非法组合；排在今天并勾上今日已做后，分别改成同一天、搁置、另一天；完成后再编辑、再勾今日已做，然后恢复；页面读取排在昨天的任务；从菜单搁置一个任务。

必须成立：收件箱的排期请求返回 400 并指出 `plan`，完成返回 409；数据库拒绝每一种非法组合；同一天保留今日已做，换成别的排期就清掉，做过的天数不变；完成清除排期，已完成任务的编辑和今日已做返回 409，恢复后为未排期；页面把过去的日子显示为未排期、可以再安排；搁置后标题变淡并写出「搁置」。

禁止结果：一个任务同时有两种排期；收件箱任务出现在安排今日的候选里。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 收件箱不接受排期，字段校验与错误字段名（领域单元测试） | Covered | `crates/domain/src/todo.rs::the_inbox_takes_no_plan`、`crates/domain/src/todo.rs::texts_are_trimmed_and_bounded` |
| 检查约束、换排期清掉今日已做、完成清除排期、已完成拒绝编辑（真实 PostgreSQL） | Covered | `crates/db/tests/postgres/todos.rs::a_task_must_keep_one_plan_and_the_inbox_none` |
| 接口上的 400 与 409（真实 Router） | Covered | `crates/backend/tests/http.rs::authenticated_http_imports_preserve_scope_and_file_parity`（经 `exercise_todos`） |
| 过去的日子读作未排期，排期的简短说法（前端单元测试） | Covered | `apps/palace-web/src/lib/todo.test.ts` 的 “reads a lapsed day as no plan and names the rest in short words” |
| 安排今日不列出收件箱和搁置的任务，菜单搁置后变淡（组件测试，请求经 mock fetch） | Covered | `apps/palace-web/src/pages/todo.test.tsx` 的 “plans the day in place from every list but the inbox”、“shelves a task from its menu and shows it faded with its word” |

决策依据：D2、D3、D6，不变量 2、3。

## Done for the day must count each day once

风险：重复点击今日已做让天数多加；取消时没有减回去，或减成负数；页面在午夜后仍停在昨天，把今天的任务记到昨天；安排今日覆盖了已经排到以后的任务，或者跳过了排在过去的任务。前置：真实 PostgreSQL、真实 Router、组件测试。触发：对同一任务连续两次设为已做、两次设为未做；带着昨天的日期设已做；对收件箱任务设已做；一次安排排在昨天（已做）、未排期、搁置、明天、收件箱五个任务；页面上点击今日的圆圈。

必须成立：天数只在从未做变为已做时加一、从已做变为未做时减一；日子与排期不符时返回 409；安排今日只改变排在昨天和未排期的两个任务，并清掉它们的今日已做，搁置、明天、收件箱的任务不变；页面上点击圆圈后发出带当天日期的请求，「已做 / 总数」随之更新。

禁止结果：做过的天数为负，或因重复请求多计；跨午夜的旧页面改动了不在当天的任务。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 计数幂等、日期校验与安排今日的筛选（真实 PostgreSQL） | Covered | `crates/db/tests/postgres/todos.rs::done_for_the_day_must_count_each_day_once` |
| 接口上的计数与 409（真实 Router） | Covered | `crates/backend/tests/http.rs::authenticated_http_imports_preserve_scope_and_file_parity`（经 `exercise_todos`） |
| 页面带上当天日期，计数随之更新（组件测试，请求经 mock fetch） | Covered | `apps/palace-web/src/pages/todo.test.tsx` 的 “opens 行事 on today, wider, and ticks a task done for today” |

决策依据：D3、D6，不变量 4。

## Task order must come from unique well-formed keys

风险：服务端接受了 `fractional-indexing` 不认的键，下一次在它旁边插入时页面抛错；同一位置出现两个相同的键，之后无法插到它们之间；数据库按区域规则而不是字节排序，与页面的顺序不一致；拖动改写了多行，或者拖过首行、末行时落不到两端；键盘无法拖动。前置：领域单元测试、真实 PostgreSQL、前端单元测试、真实 Chromium。触发：提交合法与不合法的键；在同一集合和收件箱里用已占用的键新建、移动、恢复；完成后复用它的键；在集合里把任务拖到首行之上很远、末行之下很远；用键盘拿起一行下移一格。

必须成立：只接受库能生成的格式，拒绝空键、整数部分不完整、小数以 `0` 结尾、保留的最小整数和超过 1 KiB 的键；同一位置未完成任务的键冲突返回 409，不同集合、已完成任务不冲突；数据库列使用 `C` 排序规则；每次拖动只写被拖动的那一条，键落在新邻居之间或两端之外；越界的拖动落在相应的一端。

禁止结果：一次移动改写其他任务的键；页面顺序与服务端顺序不一致。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 键格式与库一致（领域单元测试） | Covered | `crates/domain/src/todo/position.rs::keys_follow_the_library_format` |
| 未完成任务的键在同一位置唯一，`C` 排序规则（真实 PostgreSQL） | Covered | `crates/db/tests/postgres/todos.rs::open_tasks_in_one_place_must_not_share_a_key` |
| 新邻居之间、两端之外的键（前端单元测试，真实 `fractional-indexing`） | Covered | `apps/palace-web/src/lib/todo.test.ts` 的 “writes one key between the new neighbours, at either end too” |
| 指针越界落在两端、键盘拖动，每次只写一条（真实 Chromium，接口经 route mock） | Covered | `apps/palace-web/e2e/todo.spec.ts` 的 “dragging past either end lands there and writes only the moved task's key”、“the keyboard carries a row and writes a key between its new neighbours” |

决策依据：D4、D7，不变量 5。

## List order must come from unique well-formed keys

风险：集合的键不合法或重复，之后无法插到它们之间；一次移动或改名改写了其他集合；改名时丢了键，集合跳到别处；新建的集合不在末尾；收件箱被拖动或被集合越过；放下被拖动的集合时浏览器跟随链接整页刷新，打开了被拖动的集合；`Enter` 不再能打开集合；迁移打乱了已有集合的顺序。前置：真实 PostgreSQL、真实 Router、组件测试、真实 Chromium。触发：用已占用和不合法的键新建、移动集合；另一个 Owner 使用同一个键；把集合移到最上方、在原位改名；用「＋」新建并改名；用指针把末尾的集合拖到收件箱之上很远处；用键盘拿起一个集合上移一格；在集合链接上按 `Enter`。

必须成立：同一 Owner 的集合键冲突返回 409，不同 Owner 不冲突，格式不合法返回 400 并指出 `position`；看板按键的字节序排列集合；新建的键在最后一个集合之后，改名带回原来的键；拖动只写被拖动的集合，键落在新邻居之间或两端之外；收件箱始终在所有集合之前；放下后地址不变；`Enter` 打开集合。

禁止结果：一次移动改写其他集合；放下后整页刷新或打开被拖动的集合；页面顺序与服务端顺序不一致。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 集合键在同一 Owner 内唯一、跨 Owner 不冲突，`C` 排序规则，移动与改名只改写一行（真实 PostgreSQL） | Covered | `crates/db/tests/postgres/todos.rs::lists_must_not_share_a_key` |
| 接口带上 `position`，冲突 409、格式错误 400（真实 Router，OpenAPI 校验） | Covered | `crates/backend/tests/http.rs::authenticated_http_imports_preserve_scope_and_file_parity`（经 `tests/http/todos.rs::exercise_todos`） |
| 新建取末尾之后的键，改名带回原键（组件测试） | Covered | `apps/palace-web/src/pages/todo.test.tsx` 的 “adds a list after the others and renames it in place, keeping its key” |
| 指针越界落在收件箱之下、键盘拖动、只写一条、放下不跳转、`Enter` 打开（真实 Chromium，接口经 route mock） | Covered | `apps/palace-web/e2e/todo.spec.ts` 的 “lists drag below the fixed inbox, and a drop neither opens the list nor writes another” |
| 迁移按创建顺序回填键 | Missing | 只在本地 PostgreSQL 上手工核对过，没有自动化测试 |

决策依据：[集合按拖动的顺序排列](../../../decisions/web/todo/20261031-lists-in-dragged-order.md) D1、D2，不变量 1–4。

## A list must take its tasks with it

风险：删除集合后任务留在表里，`list_id` 指向不存在的集合；只删了未完成的，已完成的成了孤儿；清空已完成时误删了未完成的任务或别的集合的任务。前置：真实 PostgreSQL；两个集合，其中一个有未完成和已完成的任务。触发：读取已完成列表；清空；再完成一条、新建一条后删除集合。

必须成立：已完成列表只含这个集合的已完成任务；清空后未完成的任务和别的集合不受影响；删除集合后它的所有任务都消失，另一个集合原样保留。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 清空只删已完成，删除集合级联删除全部任务（真实 PostgreSQL） | Covered | `crates/db/tests/postgres/todos.rs::a_list_must_take_its_tasks_with_it` |

决策依据：D1、D3、D5，不变量 6。

## Undoing must leave the server as if nothing happened

风险：删除后点了「撤销」，请求其实已经发出，任务再也回不来；或者撤销后过一会儿删除仍然发出；新的提示替换旧提示时，被替换的删除永远没有发出。前置：组件测试，请求经 mock fetch。触发：删除一条任务后点「撤销」；再删除一条，接着完成另一条。

必须成立：撤销之前没有任何请求，撤销后任务原样回到列表；被新提示取代的删除在新提示出现时发出。

禁止结果：撤销的删除到达服务端；页面上消失的任务在服务端仍然存在且永远不会被删除。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 撤销不发请求，被取代的删除随之发出（组件测试） | Covered | `apps/palace-web/src/pages/todo.test.tsx` 的 “sends a deletion only once its notice is gone, and never after 撤销” |
| 提示消失后才发出删除（真实计时器） | Missing | 组件测试只覆盖被取代的路径；4 秒计时的路径只在手动浏览器验证中走过 |

决策依据：D7。

## Todo navigation must open the roomy todo page

风险：「行事」仍不可点击，或者进入后页面宽度与其他页面相同，左列挤压任务；导航没有把行事标为当前项。前置：组件测试。触发：从时刻按 Tab 经过导航；打开 `/todos`。

必须成立：导航中行事是指向 `/todos` 的链接，进入 Tab 顺序；行事页的纸面带 `roomy`，行事为当前项。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 导航链接与 Tab 顺序（组件测试） | Covered | `apps/palace-web/src/app.test.tsx` 的 “marks only 时刻, keeps unbuilt sections out of reach, and links cards to the reading page” |
| 加宽的纸面与当前项（组件测试） | Covered | `apps/palace-web/src/pages/todo.test.tsx` 的 “opens 行事 on today, wider, and ticks a task done for today” |

决策依据：D1。
