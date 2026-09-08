# 来源会话与消息树核心测试用例

本文跟踪[来源会话与消息树根决策](../../../decisions/server/conversation/0-source-session-and-message-tree.md)中身份隔离、树结构和安全跳转的长期风险。当前尚未实现，全部直接证据均为 `Missing`。

## Same source session must resolve to one conversation within an authorization scope

### 风险

同一来源会话因标题变化或重复导入生成多份 Conversation，或者不同授权范围的数据因相同 `source/session_id` 被错误合并。

### 前置状态

一个 Owner Scope 内已经存在 source 为 chatgpt、session_id 为 S、标题为 T1 的 Conversation；另有一个独立 Owner Scope。

### 触发

在原范围以标题 T2 再次定位 S，并在另一范围提交同样的 source/session_id。

### 必须成立

原 Owner Scope 复用既有 Conversation 且允许标题作为可编辑元数据变化；另一 Owner Scope 获得独立身份。Palace ID 不因标题或来源链接模板变化而改变。

### 禁止结果

不得在原范围仅因标题不同创建重复来源会话，不得跨授权范围返回或关联已有 Conversation。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| `(source, session_id)` 在同一 Owner Scope 定位唯一 Conversation | Missing | 尚无实现测试 |
| 标题变化不改变 Conversation 身份 | Missing | 尚无实现测试 |
| 相同外部身份不能跨授权范围合并 | Missing | 尚无实现测试 |

### 决策依据

根决策 D1 及不变量 1。

## Divergent paths must share their exact message prefix

### 风险

分叉导入复制完整正文、丢失后缀，或把不同内容误合并，使 `A-B-C` 与 `A-B-D` 无法恢复。

### 前置状态

Conversation 已包含路径 `A-B-C`。

### 触发

向同一 Conversation 加入路径 `A-B-D`，随后分别从 C、D 回溯并展开整棵树。

### 必须成立

A、B 各只有一个共享 Message；C、D 是 B 的不同子消息；两个叶子都能恢复唯一的完整祖先路径。若两条路径第一条消息即不同，它们都以 Conversation 作为虚拟根。

### 禁止结果

不得复制 A、B，不得把 C 覆盖为 D，不得产生父链环或无法归属于 Conversation 的根节点。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 分叉只新增不同后缀并保留共享前缀 | Missing | 尚无实现测试 |
| 任一叶子沿父链恢复唯一 Path | Missing | 尚无实现测试 |
| 从首条消息分叉时 Conversation 作为共同虚拟根 | Missing | 尚无实现测试 |

### 决策依据

根决策 D3、D4 及不变量 3、5。

## Message parent links must remain inside one acyclic conversation tree

### 风险

跨 Conversation 父子引用或环使授权边界失效，并让路径读取无法终止。

### 前置状态

存在两个 Conversation 及各自 Message，并准备自引用、祖先回指和跨 Conversation 三类父节点写入。

### 触发

分别提交三类非法关系，同时提交合法的连续 user、连续 assistant 和以 user 结束的路径。

### 必须成立

非法父节点写入均被拒绝且不改变已有树；合法路径按提交顺序保存，不要求角色交替或 assistant 结尾。

### 禁止结果

不得仅靠应用层读取习惯容忍跨对话引用或环，不得自动删除、合并或补齐连续同角色消息。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 跨 Conversation 父消息被拒绝 | Missing | 尚无实现测试 |
| 自引用和祖先回指被拒绝 | Missing | 尚无实现测试 |
| 非交替角色与 user 结尾保持原顺序 | Missing | 尚无实现测试 |

### 决策依据

根决策 D3 及不变量 3、4。

## Original conversation links must use only controlled source templates

### 风险

恶意或畸形 session_id 将跳转引向非允许域名，或者模板变化误改来源会话身份。

### 前置状态

三种受支持 source 各有一个有效 session_id，并准备含路径分隔、查询串或外部 URL 的恶意输入。

### 触发

生成原始会话跳转链接并更新其中一个 source 的模板配置。

### 必须成立

链接域名和路径框架来自 source 的受控模板，session_id 经路径段校验与编码；模板更新后 Conversation、source、session_id 和 Palace ID 保持不变。

### 禁止结果

不得接受用户提供的任意完整 URL，不得让 session_id 改写 host、scheme 或模板路径，不得因模板更新迁移业务身份。

### 验证义务与证据

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 三种 source 只使用各自受控模板 | Missing | 尚无实现测试 |
| session_id 不能逃逸模板路径段 | Missing | 尚无实现测试 |
| 模板更新不改变来源会话身份 | Missing | 尚无实现测试 |

### 决策依据

根决策 D2 及不变量 2。
