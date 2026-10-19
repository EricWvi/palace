---
status: implemented
date: 2026-10-19
---

# 札记以 Git 仓库中的 Markdown 文件为唯一权威，服务端按 commit 索引进 PostgreSQL，移动端只拉取变更和上传新札记

札记（Jotting）是一类标题可选的短文，以带 frontmatter 的 Markdown 文件保存在一个 Git 仓库中，允许多级子目录。本地 Agent 是主要写入者，直接用文件工具和 rg 维护，并以 commit 作为“改完了”的边界。服务端不扫描工作区，只监听当前分支的 commit：commit 变化时，把该 commit 中的札记与 PostgreSQL 索引对账，并为每张变化的札记分配同步版本。移动端离线保存全部札记，通过增量接口追上服务端；它只能新建札记，不能修改或删除。上传的札记由服务端写入 `inbox/`，立即进入索引，之后由 Agent 整理和 commit。

当前为 `implemented`，核心测试用例见[札记同步](../../../test-cases/server/jotting/jotting-sync.md)。本文件是 `server/jotting` 的根决策，没有前序 ADR。涉及 `palace-domain`（札记解析与校验）、`palace-db`（札记索引与同步表）、新增的 `palace-jottings` crate（Git 读取、对账与 inbox 写入）、`palace-backend` 的札记接口、`palace-server` 的配置、`pal` CLI 的校验命令，以及运行镜像的挂载要求。Web 回响页的界面、附件读取、札记之间的引用语法不在本决策范围内，见“本决策未解决的问题”。

## 问题与约束

- 札记由 Agent 用文件工具批量创建、改写、移动和删除，文件是唯一权威，Git 保存历史。Palace 不另做一份数据库真相，也不做编辑器。
- 移动端需要离线阅读和搜索，离线时也能新建札记；它没有 Git，也不关心目录结构。
- 服务端必须能告诉移动端“自上次以来哪些札记被创建、修改或删除”，而且服务重启后游标和删除记录依然有效，所以索引和 tombstone 必须持久化，不能像笔记那样放在内存里。
- Agent 的操作可能写到一半；如果没有操作，服务端也不应该周期性地全量扫描、空转。
- 移动端上传可能超时重试；不同设备可能生成相同的 ID；上传的札记写入文件后，可能还没被 commit 就已经要同步给其他设备。
- Palace 实际只有一个用户。札记和笔记一样是部署级内容，不属于某个 Owner。
- 现有的[记录同步根决策](../sync/0-client-timestamp-lww-and-global-seq.md)已经确立了“`serverVersion` 只负责发现增量、发布在同一把锁内完成”的做法，本决策复用这套发布规则，但札记不进入 `sync_record`，也不使用 LWW。

## D1：札记文件的格式与身份

札记目录由 `PALACE_JOTTINGS_DIR` 指定（见 D6），其下（含子目录）每个 `.md` 文件都是一张札记，没有例外。文件格式：

```markdown
---
id: 0199f2a4-7c1e-7b3a-9d2e-4f6a8b0c1d2e
created: 2026-10-19T08:30:00+08:00
title: 可选标题
tags: [阅读, rust]
---

正文 Markdown。
```

| 字段 | 规则 |
| --- | --- |
| `id` | 必填，小写连字符形式的 UUIDv7 |
| `created` | 必填，带时区偏移的 RFC 3339 时间，精度不超过毫秒 |
| `title` | 可选；出现时是非空字符串 |
| `tags` | 可选；字符串数组，元素非空、不重复 |

其他 frontmatter 字段原样保留在文件中，服务端不收集。正文是 frontmatter 之后的全部内容，原样保存，不做规范化。

- **身份是 `id`，不是路径。** Agent 会把札记从 `inbox/` 移到别的目录；如果像笔记那样用路径当身份，移动一次就会变成“删一张、新建一张”，移动端的离线副本和其他札记对它的引用都会断掉。路径只是服务端内部记录的位置，不进入同步接口。
- `id` 必须是 v7：移动端离线生成 ID，v7 的时间前缀让同一毫秒以外的碰撞几乎不可能；要求 Agent 写入的 ID 也是 v7，两种来源的 ID 就只有一种规则。
- `created` 是札记的业务时间，列表和同步都用它排序，不用文件 mtime 或 commit 时间，理由与笔记 D4 相同：反复修改不应让札记漂移。

## D2：服务端只按 commit 索引，不读工作区

服务端用 gitoxide 读取仓库（容器内不需要 `git` 可执行文件，也从不写 `.git`）。索引对应的是**当前分支 HEAD 所指 commit 的树**中、`PALACE_JOTTINGS_DIR` 对应前缀下的全部 `.md` 文件，加上 D4 中尚未进入任何 commit 的上传札记。

- **何时对账**：服务启动时一次；之后监听 `.git` 内 HEAD 和当前分支 ref 的文件事件，HEAD 指向的 commit 变化时对账一次。另外每分钟读一次 HEAD 的 commit ID 作为兜底，覆盖 inotify 不触发的挂载盘；commit ID 没变就什么也不做，不读任何树或文件。
- **对账算法**：遍历该 commit 中前缀下的树条目，得到 `(路径, blob OID)` 列表。索引中每张札记都记录了它的 blob OID；OID 已在索引中出现的条目不读内容，只在路径变化时更新路径。只有新的 OID 才读取 blob 并解析。所以对账的成本是“遍历一次树 + 读取变化的文件”，与 commit 之间改了多少文件成正比，不需要依赖旧 commit 仍然存在，也不需要做 rename 检测。
- **已对账的 commit 只记在内存中**：重启后与 HEAD 对账一次，已有 blob 不重新读取，所以不需要持久化。同一个 HEAD 只对账一次，被拒绝的 commit 也不会每分钟重复对账、重复报错，直到 HEAD 移动。
- **为什么是 commit**：commit 是 Agent 主动给出的完成边界，读取的是 Git 对象而不是工作区，写到一半的文件不会被看到；没有 commit 就没有任何工作。HEAD 回退（`reset`、切换分支）也一样是“与新树对账”，不需要特殊处理。代价是 Agent 改完必须 commit，否则移动端看不到。

对账在一个 PostgreSQL 事务内完成，并在 D5 的发布锁内为每张实际变化的札记分配版本：

| 新树中的情况 | 索引中的情况 | 结果 |
| --- | --- | --- |
| 有该 `id` | 没有 | 创建，发布 |
| 有该 `id`，内容与索引一致 | 有（含上传后尚未 commit 的） | 只更新路径或“已入 commit”状态，不发布 |
| 有该 `id`，内容不同 | 有 | 修改，发布 |
| 有该 `id` | tombstone | 复活为普通札记，发布 |
| 没有该 `id` | 已入 commit 的札记 | 删除，保留 tombstone，发布 |
| 没有该 `id` | 上传后尚未入 commit 的札记 | 见 D4 |

“内容一致”指 `created`、`title`、`tags`、正文都相同。只改了其他 frontmatter 字段、空白或路径时不发布，因为移动端拿到的字段没有变化。

## D3：一个 commit 不合格，整个 commit 都不索引

对账前先校验新树中的全部札记：任何一个文件 frontmatter 解析失败、字段不合 D1、或两个文件的 `id` 相同，这个 commit 整体不索引。索引和已索引的 commit ID 都保持在上一个合格的 commit，记录一条列出全部问题的错误日志。之后 Agent 修复并再次 commit，服务端与新树对账，一次追上中间的所有变化。

- 和笔记 cache 整份拒绝是同一个原则：同步出去的状态总是某个 commit 的完整状态，不会出现“这个 commit 的一部分”。
- 代价是一个坏文件会挡住同一 commit 里所有其他札记。为了让问题在 commit 之前暴露，`pal jottings check <目录>` 用与服务端同一份 Rust 解析代码（`Jotting::parse` 与 `JottingCensus`）校验工作区，可以作为 pre-commit hook。校验逻辑只有一份实现，不会出现“hook 通过、服务端拒绝”。
- `pal jottings check` 校验工作区中除 `.git` 以外的全部 `.md`，包括未跟踪的文件，按路径排序后判定，与服务端遍历 commit 树的顺序一致，因此 ID 重复时两边指出的“先占用者”相同。它不需要 `PAL_URL` 和 `PAL_TOKEN`，不发出任何请求。

## D4：移动端上传新札记，服务端写入 `inbox/`，按 ID 幂等

`POST /api/jottings` 的请求体是 `{ id, created_at, title?, tags?, body }`，`created_at` 是 epoch 毫秒，其他字段的规则同 D1。服务端在 D5 的发布锁内：

1. 按 `id` 查找索引（含 tombstone）。
2. 不存在：把请求渲染为规范化的 Markdown 文件（`created` 按 `PALACE_TIMEZONE` 在该时刻的偏移写成 RFC 3339，字段顺序固定），以临时文件加 rename 的方式原子写入 `<PALACE_JOTTINGS_DIR>/inbox/<id>.md`；然后插入索引行，状态为“已上传、尚未入 commit”，并发布。返回 `201` 和 `{ id, version }`。
3. 已存在，并且这一行本身来自上传、首次上传的内容摘要与本次请求相同：这是重试。返回 `200` 和该札记当前的版本，不写文件、不发布，无论它之后是否已被 Agent 修改、移动或删除。
4. 其他情况（ID 由 Agent 创建，或上传内容不同）：返回 `409 jotting_id_conflict`，不写文件。`inbox/<id>.md` 已经存在但内容不同时（例如 Agent 抢先写了同名文件）同样返回 `409`，服务端不覆盖它。

请求先规范化再处理：去掉正文开头的空行，使渲染出的文件解析回来与请求完全相同，Agent 原样 commit 时不会产生多余的版本；`title` 为空白、`tags` 重复或为空白、`id` 不是 v7 时返回 `400`，正文超过 1 MiB 时返回 `413`。

内容摘要是规范化后的 `id`、`created_at`、`title`、`tags`、`body` 的 SHA-256，与渲染出的文件无关。评审稿写的是“规范化文件摘要”，实现时改为内容摘要：文件中的 `created` 依赖 `PALACE_TIMEZONE`，部署改了时区之后，同一次上传的重试会渲染出不同的文件而被误判为冲突。

- **幂等键是 ID，判定依据是上传内容的摘要，而不是当前内容。** 札记上传后可能很快就被 Agent 改写；如果拿当前内容比较，重试就会被误判为冲突。索引行因此保存首次上传的摘要，并且永不修改。
- **已删除的札记不会被重试复活**：tombstone 保留上传摘要，重试走第 3 步。
- **文件先写、行后写**：行存在就意味着文件已写入。插入行失败时文件留在 `inbox/`，重试时第 2 步发现内容相同的文件已在，直接保留它并插入行；不会产生第二张札记。万一一直不重试，这个文件会在 Agent 下一次 commit 时作为普通札记进入索引，结果也正确。
- **服务端不 commit。** 服务端如果去改 index 和 HEAD，会和 Agent 的工作区抢锁，甚至让 Agent 的下一次 commit 把这个文件当作被删除。所以上传的札记先只存在于工作区和索引中，等 Agent commit。

**尚未入 commit 的上传札记**（D2 表最后一行）：对账时，如果新树里没有它，就检查 `inbox/<id>.md` 是否还在工作区。在的话保持不变，继续等待 commit；不在的话说明 Agent 在 commit 之前就删除了它，于是写成 tombstone 并发布。新树中一旦出现它的 `id`，它就转为“已入 commit”，此后按普通规则处理。

已知边界：Agent 把上传札记移到别的目录但还没 commit 时，如果恰好另有一次 commit 触发对账，`inbox/<id>.md` 已经不在，札记会被暂时删除；等移动后的文件被 commit，又会按“tombstone 复活”重新发布，最终状态正确。Agent 通常会把移动和 commit 放在一起做，这里接受一次多余的删除再复活，而不是为了它去扫描工作区。

## D5：同步版本、增量接口与 tombstone

- 札记索引表的每一行（含 tombstone）带一个 `server_version`。版本来自一个新的 PostgreSQL sequence `jotting_server_version`，取号和写入都在同一把事务级 advisory lock 内完成，规则与记录同步根决策的 D4 相同：持锁到提交，禁止预先取号，锁内不做网络请求。D4 的本地文件写入允许在锁内进行，它只是本地 rename，而且必须在发布前完成。
- 不与 `sync_record` 共用 sequence 和锁：两边的游标互不相干，共用只会让两套同步互相等待。
- tombstone 永久保留，第一版不清理。

`GET /api/jottings/changes?since=<版本>&limit=<n>` 按 `server_version` 升序返回 `server_version > since` 的札记当前状态，响应为 `{ changes, next, has_more }`：

| 字段 | 含义 |
| --- | --- |
| `changes[]` | `{ id, version, deleted }`；`deleted` 为 `false` 时另有 `jotting: { id, created_at, title, tags, body }`，`created_at` 是 epoch 毫秒 |
| `next` | 本页读到的最后一行的版本；空页时等于 `since` |
| `has_more` | 之后是否还有行 |

- 版本用十进制字符串传输，与 `ServerVersion` 一致。`limit` 默认 200，上限 1000，按读取的行数计算，包括首次同步中被略去的 tombstone。
- 客户端只能把游标推进到这一页的 `next`，并且必须和本页的应用写在同一个本地事务里。空页时 `next` 等于 `since`。
- **不单独提供 snapshot 接口**：首次同步就是 `since=0`。在 `since=0` 时，服务端不返回 tombstone，因为新设备上没有需要删除的东西；返回的 `next` 仍然覆盖这些被跳过的版本。discussion 中设想的 `GET /sync/snapshot` 因此合并进本接口。
- 移动端已提交的上传会以普通变更出现在后续拉取中；客户端收到自己上传的 ID 时，用服务端版本覆盖本地副本并结束待上传状态。“本地新建但尚未上传的札记不能被拉取结果覆盖”：本地待上传的 ID 只可能与服务端冲突（D4 第 4 步返回 409），不会被正常拉取覆盖，因为服务端在接受上传之前不会有这个 ID。

## D6：配置与运行条件

- 新增可选环境变量 `PALACE_JOTTINGS_DIR`，指向容器内的札记目录。它必须位于一个 Git 工作区内（可以是仓库的子目录），容器必须能看到整个仓库，包括 `.git`；服务端对 `inbox/` 必须有写权限。
- 未设置时，札记功能不可用：`changes` 返回空页，`POST` 返回 `503 jottings_unavailable`。已设置但不是 Git 工作区、HEAD 无法解析，或分支处于 detached HEAD 时，启动失败，并指出变量名和原因。这里与笔记“没有 cache 也能启动”不同：札记是可写入的同步源，配置错了却静默运行，会让上传写进一个永远不会被索引的地方。
- 仓库属主与容器用户不同时，gitoxide 默认的属主检查会拒绝打开；服务端显式信任这个已配置的路径，并以隔离模式打开，不应用仓库和全局的 Git 配置，只读取对象和 ref。
- 启动时先与 HEAD 对账一次。数据库或仓库不可用导致对账失败时启动失败；commit 不合格只记录错误日志，服务照常启动，索引停在上一次的状态。

## D7：访问范围

札记不属于任何 Owner，表上没有 `owner_id`。所有已认证请求（浏览器 Session 或 Access Token）都可以拉取变更和上传，未认证请求被拒绝。Palace 实际只有一个用户，为它设计 Owner 隔离不会带来任何收益；这与笔记 D3 的处理一致，也就不受 Owner 根决策中“业务表必须带 `owner_id`”这条约束的限制。

## 不变量

1. 札记的身份只有 frontmatter 中的 `id`；移动或改名文件不会改变身份，也不会产生删除和创建两条变更。
2. 已发布的札记状态总是某个合格 commit 的树，再加上尚未入 commit 的上传札记；工作区中未 commit 的修改不会被发布。
3. 一个不合格的 commit 不会改变索引中的任何一行，也不会推进已索引的 commit。
4. 同一个 `id` 以相同内容重复上传，无论间隔多久、中间发生了什么，最多只创建一张札记、一个文件，而且不会复活已删除的札记。
5. 以不同内容上传已存在的 `id`，不会修改任何文件或索引行。
6. 索引中每一行上传札记，都对应一个已经写入的 `inbox/` 文件，或者一个包含它的 commit。
7. 服务端从不写 `.git`，也从不修改或删除 `inbox/<id>.md` 以外的任何文件。
8. 札记版本在发布锁内分配并与索引原子提交；客户端按 `next` 推进游标不会漏掉任何已提交的变更。
9. 删除以 tombstone 保留，`since > 0` 的拉取一定能看到它。

## 为什么不是这些替代方案

| 替代方案 | 优势 | 取舍 |
| --- | --- | --- |
| 服务端 SQLite + FTS5（discussion 初稿） | 与移动端同构；FTS5 自带全文检索 | 要在 PostgreSQL 之外再维护一个库和一套发布锁、游标与备份；中文分词在 FTS5 上同样需要额外处理。现有 PG 同步规则可以直接复用 |
| 内存索引 + rg（同笔记） | 最简单，无需建表 | 重启后版本和 tombstone 丢失，增量同步不成立 |
| 每分钟全量扫描工作区，或递归 watch + 防抖 | 不要求 Agent commit | 没有变化时也在空转；仍然可能读到写了一半的文件；挂载盘上 inotify 不可靠 |
| Agent 写完后通过 `pal` 调用 reindex 接口 | 不依赖 Git | 漏调就不同步；“写完”的边界与 Git 历史是两件事，需要分别记得 |
| 用 `git diff old..new` 加 rename 检测 | 只看变化的路径 | 依赖旧 commit 仍然存在（rebase、gc 后可能消失），而且 rename 检测是启发式的；遍历树加 blob OID 对账不需要旧 commit，也不需要猜测 |
| 用路径作为身份（同笔记） | 不需要在 frontmatter 中手写 `id` | Agent 整理 inbox 就是在移动文件，路径身份会让每次整理都变成“删除 + 创建”，引用也随之断掉 |
| 服务端在上传后自己 commit | 文件与 Git 历史时刻一致 | 与 Agent 争用 index 和 HEAD，可能让 Agent 的下一次 commit 删除这个文件；服务端还需要 `.git` 的写权限 |
| 一个坏文件只跳过该文件 | 一个错误不影响其他札记 | 同步状态不再对应任何一个 commit，坏文件修好之前的状态很难说清；pre-commit 校验已经能在 commit 之前发现问题 |
| 单独提供 snapshot 接口 | 首次同步可以一次取完 | 与 `since=0` 的增量拉取语义重复，还需要额外保证快照与游标一致；省略 tombstone 后两者的成本相当 |
| 复用 `sync_record` 和 LWW | 少一张表 | 札记不在客户端编辑，没有 LWW 的需求；`sync_record` 带 `owner_id` 并且按 Owner 分区 |

## 风险与为什么不能直接改写

- **版本与身份不能重置**：投入使用后不能重置 `jotting_server_version`，也不能清空 tombstone 或改变 `id` 规则，否则移动端的游标和离线副本都会失效。需要时，先写后续 ADR 定义重建流程，让客户端丢弃本地副本、从 `since=0` 重新同步，同时保留待上传队列。
- **数据库恢复到旧备份**：版本历史会倒退，旧游标会漏掉变更，此时必须让所有客户端重建，理由同上。
- **Agent 修改 `inbox/` 中尚未 commit 的上传札记**：修改在 commit 之前不会同步，这符合不变量 2。
- **文件权限**：服务端写入的 `inbox/` 文件属于容器用户。部署时要让宿主机上的 Agent 能够移动和修改它们，例如统一 UID 或设置组权限。
- **大仓库**：每次对账都遍历札记前缀下的整棵树。千级文件时，这只是一次内存中的树遍历；札记前缀应当是仓库中的一个子目录，而不是巨型单仓的根目录。

## 本决策未解决的问题

- **Web 回响页**：列表、搜索、随机回顾等读取接口，由回响页的 Web 决策定义；本决策只提供同步接口和上传接口。
- **附件**：札记中的图片和附件如何引用媒体文件、如何获得访问地址（discussion 中的 `GET /attachments/:id`），依赖媒体根决策中尚未解决的读取接口，由后续决策定义。
- **札记之间的引用**：正文中引用其他札记的语法（例如以 `id` 为目标的链接）。本决策保证引用目标的 `id` 稳定，但服务端不解析引用。
- **移动端本地存储与待上传队列**：客户端的 SQLite 表结构、上传重试和冲突展示，由移动端决策定义。本决策只规定协议层的义务（D5）。
- **tombstone 清理与协议升级**：同记录同步根决策，第一版不处理。

## 落地与验收

- `palace-domain` 的 `Jotting` 负责 D1 的解析、上传规范化、渲染与内容摘要，`JottingCensus` 负责整组文件的判定；服务端与 `pal jottings check` 共用两者。
- migration `0016_jottings.sql` 新增 `jotting` 表、`jotting_server_version` sequence、发布触发器与禁止删除触发器：版本在触发器内取得发布锁后分配，只有客户端可见的内容变化才分配新版本，数据库层面保证不变量 8、9。`palace-db` 的 `JottingTransaction` 是写札记的唯一入口，从第一条语句起持有发布锁。
- `palace-jottings` 用 gitoxide 读取 commit 树，以纯函数规划对账步骤，在一个事务内执行；上传写入 `inbox/` 后插入行；监听 git dir、common dir 与 `refs/heads`，并每分钟读一次 HEAD。
- `palace-backend` 挂载 `GET /api/jottings/changes` 与 `POST /api/jottings`，纳入 OpenAPI 合同；`palace-server` 的两个入口从 `PALACE_JOTTINGS_DIR` 构造服务并保持监听。
- 真实 Git 仓库加真实 PostgreSQL 的测试覆盖不变量 1–9，真实路由的 HTTP 测试覆盖状态码、错误码与响应形状。证据见核心测试用例。

现有的记录同步、笔记、媒体和时间线行为不变。
