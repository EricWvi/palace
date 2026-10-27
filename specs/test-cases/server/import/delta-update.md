# 增量更新核心测试用例

决策：[更新分支可以“仅上传新增消息”](../../../decisions/server/import/20261010-delta-append-to-current-path.md)，继承[分支与追加导入](../../../decisions/server/import/20260919-explicit-branch-and-append-imports.md)的原子性、幂等与标题规则。完整模式的更新规则见[明确目标的线性导入](linear-path-import.md#path-updates-must-retain-the-entire-historical-prefix)，本文件只覆盖 `history_mode` 与 `delta` 模式。

## Updates must declare their history mode

风险：服务端按内容猜测模式，把历史被改写的完整 JSON 当作增量接到末尾；或增量请求缺少基准末端仍被接受。前置：一个已有 Path。触发：更新请求缺少 `history_mode`、`delta` 不带 `base_head_message_id`、`full` 携带 `base_head_message_id`、`history_mode` 取未知值。

必须成立：上述请求在任何写入前整体拒绝，返回字段错误；合法的 `full` 请求行为与本 ADR 之前完全相同。禁止服务端在完整模式失败后改按增量处理。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 领域模型无法构造“增量却没有基准末端” | Covered | 编译期类型约束：`UpdateHistory::Delta` 携带 `base_head_message_id`，没有其他增量形态 |
| HTTP 拒绝缺失或多余的模式相关字段，且无写入 | Covered | `http/paths.rs::exercise_path_lifecycle`（真实 HTTP/PG；之后 Path 消息数只计入合法增量） |
| 完整模式前缀不符时拒绝，不回退为增量 | Covered | `paths::updates_are_append_only_and_retries_preserve_path_metadata`（真实 PostgreSQL） |

决策依据：D1，不变量 1、2。

## Delta updates must extend the current head without touching history

风险：增量改写、比对或截短已有消息；或在另一 Path 已续写相同内容时新建重复兄弟；或消息数仍按 JSON 长度计算。前置：Path P1 为 U1-A1-U2-A2，另一 Path P2 为 U1-A1-U2-A2-U3-A3；来源已改写 U1 的内容。触发：对 P1 以 `delta` 提交 `[U3, A3, U4]`；对 P1 以 `delta` 提交以 user 开头、末端也是 user 的增量；提交空数组。

必须成立：P1 末端变为 U4，U3、A3 复用 P2 的消息，只新建 U4；P1 已有消息、P2 及其他 Path 不变；P1 消息数等于从根到新末端的条数；角色不交替照常写入；空数组按字段错误拒绝。标题替换、`occurred_at` 与 Moment 日期移动和完整模式一致。禁止同一父节点下出现角色与内容完全相同的两个兄弟。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 增量接在末端，复用末端下相同子消息，只新建其余消息 | Covered | `delta_updates::delta_extends_the_head_and_reuses_shared_continuations`（真实 PostgreSQL） |
| 已有消息与其他 Path 不变，消息数等于根到新末端的条数 | Covered | `delta_updates::delta_extends_the_head_and_reuses_shared_continuations`（真实 PostgreSQL） |
| 标题、`occurred_at` 与 Moment 日期随增量一起提交 | Covered | `delta_updates::delta_extends_the_head_and_reuses_shared_continuations`（真实 PostgreSQL，整条 Path 记录比对） |

决策依据：D2，不变量 3、5、6。

## Delta updates must reject a stale base head atomically

风险：重复提交或两个窗口基于同一旧末端各自追加，产生重复或交错的尾巴。前置：Path 末端为 H。触发：以 `base_head_message_id=H` 提交增量 X 成功后，再以新幂等键、同一基准提交 X；两个以 H 为基准的增量并发提交；完整更新移动末端后提交基准为 H 的增量。

必须成立：基准与锁内读到的最新末端不同时整次拒绝，返回可区分的冲突错误，不写入任何消息、标题、时间或回执；并发的两个增量恰好一个成功。禁止增量接到请求未见过的末端之后。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 过期基准拒绝且无任何写入 | Covered | `delta_updates::stale_delta_bases_are_rejected_without_writes`（真实 PostgreSQL，重复增量与完整更新后两种情形） |
| 并发增量恰好一个成功 | Covered | `delta_updates::stale_delta_bases_are_rejected_without_writes`（真实 PostgreSQL；两个请求同时提交，写锁下任一先后顺序都只能一个成功） |
| HTTP 返回冲突类别，界面提示刷新后重试 | Covered | `http/paths.rs::exercise_path_lifecycle`（真实 HTTP/PG，409 `path_head_moved`）、`pages/conversation.test.tsx::uploads only new messages after the head being read, and forgets the choice on reopening` |

决策依据：D3，不变量 4。

## Idempotency keys must not cross history modes

风险：同键下完整模式与增量模式互相充当重试，返回与请求语义不符的结果。前置：一次已提交的更新。触发：同键同正文同模式重试；同键、同 JSON 换模式；同键、同 JSON 换基准末端。

必须成立：同键同请求返回原结果且不再写入；模式或基准末端不同即冲突。禁止换模式的请求返回前一次结果。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 摘要包含模式与基准末端 | Covered | `update_history_mode_and_base_head_are_part_of_idempotency`（领域单元测试） |
| 同键跨模式冲突、同请求返回原结果 | Covered | `delta_updates::stale_delta_bases_are_rejected_without_writes`（真实 PostgreSQL） |

决策依据：D4，不变量 7。

## The new-messages-only switch must default off in update mode only

风险：开关状态被记住，下次上传完整 JSON 时整段历史被重复追加；或开关出现在新建对话、新建分支中。前置：阅读页一个 Path。触发：从标题菜单与分支列表分别打开更新对话框；打开开关提交后再次打开；打开新建对话与新建分支对话框；完整模式前缀不符失败。

必须成立：两个入口的更新对话框都有开关“增量导入”（原名“仅上传新增消息”，见[直接追加消息](../../../decisions/server/import/20261027-write-delta-messages-in-update-dialog.md) D2），每次打开都关闭；打开后说明文字给出接续位置（当前消息数），请求体为 `delta` 并携带当前末端；切换开关清除上次错误并换用新幂等键；完整模式前缀失败的提示指向该开关；新建对话与新建分支没有该开关。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 仅更新模式出现开关，每次打开默认关闭 | Covered | `pages/conversation.test.tsx::uploads only new messages after the head being read, and forgets the choice on reopening`、`components/branch-manager.test.tsx::creates a branch named after the path being read, with the source locked to the conversation`、`app.test.tsx::imports from the day menu at the viewed day and opens the reading page dated to it` |
| 开关打开时请求体与说明文字正确，切换换键并清错 | Covered | `pages/conversation.test.tsx::uploads only new messages after the head being read, and forgets the choice on reopening`；关闭时请求体为 `full`：`components/branch-manager.test.tsx::updates with the path's own title and occurrence time without sending identity fields` |
| 增量更新的请求、窄屏布局与成功落点 | Covered | `e2e/branches.spec.ts`（真实 Chromium，从分支列表打开；标题菜单与列表共用同一表单） |

决策依据：D5，不变量 8。

## Written messages must append as a delta and reset on reopening

风险：写下的消息按完整历史比对或以与文件不同的形态发送；空框被当作消息提交；方式被记住，下次上传完整文件时被当作增量重复追加。前置：阅读页一个 Path。触发：在更新对话框切到“追加消息”，添加、删除、改角色、留空提交、填写 Markdown 后提交；切回“对话文件”；关闭后再次打开；窄屏打开两种方式。

必须成立：切到“追加消息”后不显示开关与文件框，说明给出接续位置；新行默认取上一行的另一角色，角色可任选；有空内容时提示第几条且不发请求；请求体为 `delta`、携带当前末端，`history` 是由各条 `{"role","content"}` 组成的 JSON 字符串，user 内容为原文，assistant 内容只把 `\\(…\\)`、`\\[…\\]` 公式转为美元符号定界、代码不动；切回“对话文件”时开关关闭；每次打开都回到“对话文件”。390px 宽时两种方式都不横向溢出，“对话文件”一行不折行。禁止“追加消息”发出 `full` 请求。

验证义务与证据：

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 切换、默认角色、删除、空内容拒绝与请求体 | Covered | `pages/conversation.test.tsx::appends messages written in the dialog as a delta string, and returns to the file on reopening` |
| assistant 公式转换的规则（两种 ChatGPT 语法、列表与引用前缀、代码保留） | Covered | `lib/math-delimiters.test.ts::turns either ChatGPT formula syntax into dollar delimiters, leaving code alone` |
| 切回文件时开关关闭，重新打开回到“对话文件” | Covered | `pages/conversation.test.tsx::appends messages written in the dialog as a delta string, and returns to the file on reopening` |
| 窄屏下两种方式的布局 | Covered | `e2e/branches.spec.ts`（真实 Chromium，390px） |

决策依据：[直接追加消息](../../../decisions/server/import/20261027-write-delta-messages-in-update-dialog.md) D1、D2，不变量 1–5。
