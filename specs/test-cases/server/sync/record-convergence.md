# 知识记录同步核心测试用例

本文跟踪[客户端时间戳 LWW 与 PG 全局序列同步根决策](../../../decisions/server/sync/0-client-timestamp-lww-and-global-seq.md)中冲突、重试、提交顺序、游标和删除风险。实现证据按服务端与本地持久化边界分别维护。

## Record conflicts must use strict whole-record timestamp LWW

### 风险

服务端接收时间、字段级合并或 `serverVersion` 意外参与冲突裁决，导致不同于批准规则的记录胜出。

### 前置状态

接收方已有 updatedAt 为 1000 的记录，准备内容不同且时间分别为 999、1000、1001 的三个完整版本。

### 触发

依次在服务端和客户端两个接收方向应用三个版本。

### 必须成立

999 被忽略，1000 保留接收方已有完整记录，1001 整条覆盖业务字段；同步状态与本地派生字段不随业务记录覆盖。

### 禁止结果

不得按服务端接收先后、设备 ID 或 serverVersion 决胜，不得拼接两个版本的字段。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 仅严格更大的 updatedAt 覆盖已有记录 | Missing | 尚无实现测试 |
| 等值冲突保留接收方完整版本 | Missing | 尚无实现测试 |
| serverVersion 和本地状态不参与业务覆盖 | Missing | 尚无实现测试 |

### 决策依据

根决策 D2 及不变量 1。

## Upload acknowledgements must not clear newer local mutations

### 风险

迟到上传响应清除发送后产生的新编辑或删除，使新版本永久不再上传。

### 前置状态

本地持久化一个待同步版本，发送快照后分别产生 updatedAt 更大的修改和同毫秒但修改代次更大的修改。

### 触发

收到旧快照的“已接收”或“保留服务端版本”结果。

### 必须成立

确认事务同时核对 updatedAt 与本地修改代次；任一已变化时保留当前待同步状态。上传失败项继续待同步，已明确处理且未变化的版本才可确认。

### 禁止结果

不得把网络成功等同于记录写入成功，不得由旧响应覆盖或确认发送后产生的新状态。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 业务修改与待同步标记原子持久化 | Missing | 尚无实现测试 |
| 新 updatedAt 阻止旧确认清除待同步 | Missing | 尚无实现测试 |
| 同毫秒新代次阻止旧确认清除待同步 | Missing | 尚无实现测试 |
| 逐条失败不阻塞一轮后续拉取 | Missing | 尚无实现测试 |

### 决策依据

根决策 D3 及不变量 2。

## Published server versions must never become invisible behind a cursor

### 风险

PG sequence 取号与提交错序，使客户端先处理更高版本并推进游标，随后提交的较低版本永久漏拉。

### 前置状态

两个并发服务端事务准备产生有效变更；事务 A 先请求版本，事务 B 可能先提交。另准备旧版本、等值上传和回滚事务。

### 触发

并发提交、回滚并持续按 `serverVersion > cursor` 分页拉取。

### 必须成立

所有同步写入持有同一事务级发布锁直到提交或回滚；更高的新版本发布后不再发布更低的新版本。回滚可留下空洞，旧版本和等值上传不分配业务变更版本。

### 禁止结果

不得预先取号后在锁外提交，不得绕过统一入口写同步记录，不得要求客户端等待 sequence 空洞。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 并发有效写入按提交可见顺序发布版本 | Covered | `crates/db/tests/postgres.rs::sync_publication_preserves_commit_order_lww_and_owner_scope`，真实 PostgreSQL 17，默认 ignore |
| 回滚只产生允许的版本空洞 | Covered | `crates/db/tests/postgres.rs::sync_publication_preserves_commit_order_lww_and_owner_scope`，真实 PostgreSQL 17，默认 ignore |
| 被忽略或重复确认的写入不制造业务版本 | Covered | `crates/db/tests/postgres.rs::sync_publication_preserves_commit_order_lww_and_owner_scope`，真实 PostgreSQL 17，默认 ignore |
| 绕过发布锁的同步写入路径不存在 | Missing | 尚无实现测试 |

### 决策依据

根决策 D4 及不变量 3。

## Pull cursor must advance only with durable page processing

### 风险

客户端在记录或派生任务落库前推进游标，崩溃恢复后永久跳过已经被服务端认为交付的数据。

### 前置状态

服务端存在多页增量，其中包含会被本地 LWW 忽略的记录；客户端在页面处理不同阶段发生事务失败或进程崩溃。

### 触发

拉取、应用页面、持久化派生任务并推进游标，然后重启恢复。

### 必须成立

页面记录、必要派生任务和游标在同一本地事务提交；失败时整页重试。被 LWW 忽略的记录也允许推进，空页保持原游标，授权范围变化建立新同步范围。

### 禁止结果

不得使用 sequence 当前值或上传响应版本推进拉取游标，不得只提交游标而遗漏记录或任务。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 页面处理与游标推进原子提交 | Missing | 尚无实现测试 |
| 崩溃恢复重放整页且结果幂等 | Missing | 尚无实现测试 |
| 空页、忽略记录和授权范围变化符合游标规则 | Missing | 尚无实现测试 |

### 决策依据

根决策 D4 及不变量 4。

## Server tombstones must delete locally without timestamp comparison

### 风险

客户端以较新的本地时间拒绝服务端删除，或迟到上传响应在删除后重建记录。

### 前置状态

客户端有 updatedAt 高于墓碑的未上传编辑和派生任务；另有一个删除前发出、删除后返回的上传请求。

### 触发

客户端从拉取或上传冲突结果收到 `isDeleted = true`，随后收到迟到响应并重复收到墓碑。

### 必须成立

客户端不比较 updatedAt，删除本地业务记录和待同步修改，取消派生任务；重复墓碑无害，迟到响应不能复活记录。服务端继续保留墓碑供离线客户端发现。

### 禁止结果

不得保留较新的本地编辑，不得将删除后的缺失误判为可由旧响应重建，不得从增量查询中过滤墓碑。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 墓碑无条件删除本地记录和待同步修改 | Missing | 尚无实现测试 |
| 重复墓碑与迟到响应不能复活记录 | Missing | 尚无实现测试 |
| 服务端墓碑持续参与增量拉取 | Covered | `crates/db/tests/postgres.rs::sync_publication_preserves_commit_order_lww_and_owner_scope`，真实 PostgreSQL 17，默认 ignore |

### 决策依据

根决策 D5 及不变量 5。
