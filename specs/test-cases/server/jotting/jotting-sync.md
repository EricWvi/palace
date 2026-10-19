# 札记同步核心测试用例

本文跟踪[札记根决策](../../../decisions/server/jotting/0-jottings-indexed-from-git-commits.md)中札记身份、按 commit 索引、上传幂等和增量同步的长期风险。版本分配、发布顺序、tombstone 与对账原子性只能以真实 PostgreSQL 测试为直接证据；按 commit 索引的义务只能以真实 Git 仓库（由测试创建 commit）为直接证据；上传写文件的义务只能以真实文件系统为直接证据。

## Moving a jotting file must keep its identity

### 风险

Agent 把札记从 `inbox/` 移到其他目录，服务端按路径识别札记，于是发布一次删除和一次创建；移动端丢掉离线副本，其他札记对它的引用失效。

### 前置状态

索引已对齐到一个包含札记 A 的 commit。

### 触发

在新 commit 中移动札记 A 的文件（内容不变），再在另一个 commit 中同时移动并修改它；服务端对账。

### 必须成立

第一次对账不发布任何变更，只更新服务端内部记录的路径；第二次对账只为札记 A 发布一次修改，`id` 不变。

### 禁止结果

不得发布 A 的删除，也不得出现第二个 `id`；只改路径时不得分配新版本。

### 验证义务与证据

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 只移动文件不发布变更 | Missing | — |
| 移动并修改只发布一次修改，身份不变 | Missing | — |

### 决策依据

D1、D2，不变量 1。

## Only committed jotting states may be published

### 风险

服务端读取了工作区中写到一半或尚未 commit 的文件，把中间状态同步给移动端。

### 前置状态

索引已对齐到 commit C1；工作区中修改了一张札记、新增了一张札记，都没有 commit。

### 触发

在没有新 commit 时触发对账；然后 commit 这些修改，再次对账。

### 必须成立

第一次对账不改变索引，也不发布任何变更；第二次对账发布的内容与 commit 中的 blob 一致。

### 禁止结果

不得发布任何来自工作区、未 commit 的内容。

### 验证义务与证据

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| HEAD 未变时对账不读不写 | Missing | — |
| 发布内容来自 commit 树而不是工作区 | Missing | — |

### 决策依据

D2，不变量 2。

## An invalid commit must leave the index untouched

### 风险

一个 commit 中有不合格的札记，服务端只索引了其中一部分，导致同步出去的状态不对应任何 commit；或者修复之后中间的变化被跳过。

### 前置状态

索引已对齐到合格的 commit C1。

### 触发

commit C2：修改札记 A，同时加入一个缺少 `id` 的文件（另做两组：两个文件 `id` 重复；`id` 不是 UUIDv7）。对账。然后 commit C3 修复坏文件，再次对账。

### 必须成立

对 C2 的对账不改变任何索引行，已索引的 commit 仍是 C1，并记录列出全部问题的错误日志；对 C3 的对账一次发布 A 的修改和修复后的新札记。

### 禁止结果

C2 中的合格修改不得单独被发布；修复后不得漏掉 C2 中的变化。

### 验证义务与证据

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 不合格 commit 不改变索引与已索引 commit | Missing | — |
| 修复后一次追上中间的变化 | Missing | — |
| `pal jottings check` 与服务端使用同一校验，拒绝同样的文件 | Missing | — |

### 决策依据

D3，不变量 3。

## Retrying an upload must never create a second jotting

### 风险

上传超时后客户端重试，服务端再次写文件、再次发布，或在札记已被 Agent 修改、删除后，重试把它改回原样或复活。

### 前置状态

札记 X 已成功上传一次；分别准备：X 未变、X 已被 Agent 修改并 commit、X 已被删除并 commit 成 tombstone；另外准备“写文件之后、插入行之前”注入失败的首次上传。

### 触发

以完全相同的请求再次调用 `POST /api/jottings`。

### 必须成立

三种情形都返回 `200` 和当前版本，不写文件、不分配版本；注入失败的首次上传返回失败，重试返回 `201`，`inbox/` 中只有一个 `<id>.md`，索引中只有一行。

### 禁止结果

不得出现第二个文件或第二行；不得复活已删除的札记；不得覆盖 Agent 的修改。

### 验证义务与证据

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 相同内容重试返回 200 且无副作用（含已修改、已删除） | Missing | — |
| 写文件后插入行失败，重试收敛到一个文件一行 | Missing | — |

### 决策依据

D4，不变量 4、6。

## Uploading a different jotting under an existing id must be rejected

### 风险

两台设备生成了相同的 ID，或上传的 ID 与 Agent 写入的札记相同，服务端用新内容覆盖了已有札记。

### 前置状态

索引中有 Agent 写入的札记 A，以及上传的札记 B。

### 触发

以 A 的 `id` 上传任意内容；以 B 的 `id` 上传不同内容。

### 必须成立

两次都返回 `409 jotting_id_conflict`，不写任何文件，不改变索引，不分配版本。

### 禁止结果

不得修改 A 或 B 的文件或索引行。

### 验证义务与证据

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 冲突上传返回 409 且无副作用 | Missing | — |

### 决策依据

D4，不变量 5、7。

## An uploaded jotting must survive until its commit

### 风险

上传的札记还没被 commit，下一次对账发现它不在 commit 树中，就把它删除了；或者 commit 之后又发布了一次多余的修改。

### 前置状态

札记 U 已上传，`inbox/<id>.md` 存在但未 commit。

### 触发

另一次不相关的 commit 触发对账；然后原样 commit `inbox/<id>.md` 并对账；另做一组：在 commit 之前删除 `inbox/<id>.md`，再由不相关的 commit 触发对账。

### 必须成立

不相关的对账不改变 U；原样 commit 后 U 转为已入 commit，不发布变更；commit 之前被删除的 U 写成 tombstone 并发布一次。

### 禁止结果

不得在 `inbox/<id>.md` 仍在时删除 U；原样 commit 不得分配新版本。

### 验证义务与证据

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 未入 commit 的上传札记跨对账保留 | Missing | — |
| 原样 commit 不发布变更 | Missing | — |
| commit 之前删除的上传札记被写成 tombstone | Missing | — |

### 决策依据

D2、D4，不变量 2、6。

## Pulling changes by cursor must not skip any committed change

### 风险

取号与提交错序，客户端拉到更高的版本后，较低版本才提交，永远被跳过；删除没有传播到已同步的设备。

### 前置状态

两个并发事务（对账与上传）分别在发布锁内写入札记；另有已删除的札记。

### 触发

用 barrier 让一个事务持锁未提交时另一个尝试发布；客户端按 `next` 逐页拉取；分别从 `since=0` 与 `since>0` 拉取。

### 必须成立

后到的事务等待锁，版本按提交顺序单调；逐页拉取恰好看到每个已提交变更的最终状态；`since>0` 能看到 tombstone，`since=0` 不返回 tombstone，但 `next` 覆盖了这些版本。

### 禁止结果

不得出现已提交但低于某个已返回 `next` 的未见版本；不得物理删除札记行。

### 验证义务与证据

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 发布锁保证提交顺序与版本顺序一致 | Missing | — |
| 分页拉取不重不漏，tombstone 规则正确 | Missing | — |
| 札记行不能被物理删除 | Missing | — |

### 决策依据

D5，不变量 8、9。

## The server must never write outside inbox

### 风险

服务端在对账或上传时修改了 `.git`、Agent 的札记文件或 `inbox/` 以外的路径，与 Agent 的工作区冲突。

### 前置状态

札记目录位于真实 Git 仓库中；记录上传前后整个仓库（含 `.git`）的文件清单与内容摘要。

### 触发

执行一次对账和一次上传。

### 必须成立

唯一的文件系统变化是新增的 `inbox/<id>.md`。

### 禁止结果

`.git` 下任何文件不得变化；其他札记文件不得变化。

### 验证义务与证据

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 对账与上传只新增 `inbox/<id>.md` | Missing | — |

### 决策依据

D2、D4，不变量 7。

## Jotting configuration must fail fast when misconfigured

### 风险

`PALACE_JOTTINGS_DIR` 指向一个不在 Git 工作区内的目录，服务静默启动，上传写进一个永远不会被索引的地方。

### 前置状态

分别准备：未设置；指向 Git 工作区中的子目录；指向非 Git 目录；仓库处于 detached HEAD。

### 触发

构造札记服务并调用接口。

### 必须成立

未设置时 `changes` 返回空页，上传返回 `503 jottings_unavailable`；工作区子目录正常工作；后两种情况构造失败并指出原因。

### 禁止结果

配置错误不得被当作未配置。

### 验证义务与证据

| 验证义务 | 状态 | 代表性证据 |
| --- | --- | --- |
| 四种配置的结果正确 | Missing | — |

### 决策依据

D6。
