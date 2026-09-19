# ChatGPT 对话导出工作流维护说明

本目录的 [`export-conversation.automa.json`](export-conversation.automa.json) 是 Automa `1.30.02` 导出的工作流。它在当前 ChatGPT 对话页逐条触发“复制”操作，将复制结果整理为 JSON 并下载为 `chatgpt-conversation.json`。同名文件已存在时，Automa 会自动生成不冲突的文件名。

## 使用前提

1. 在 Automa 中导入工作流 JSON。
2. 打开需要导出的 ChatGPT 对话，等待回复生成结束。
3. 将对话滚动到顶部后手动运行工作流。工作流只会从当前已经挂载的消息开始向下扫描，从中间启动可能漏掉更早的虚拟列表节点。
4. 导出后检查首尾消息、消息数量以及包含代码块的消息，再把 JSON 导入 Palace。

工作流不会真正写入系统剪贴板。它会临时代理页面中的 `navigator.clipboard.writeText` 和 `navigator.clipboard.write`，直接捕获 ChatGPT 复制按钮准备写入的纯文本。

## 总体流程

```text
手动触发
  → 绑定当前标签页
  → 校验 ChatGPT 消息节点
  → 安装剪贴板拦截器
  → 初始化变量和页面内状态
  → 循环查找未处理的 turn
      ├─ 找到：悬停 → 检查复制按钮
      │          ├─ 有按钮：点击 → 捕获复制文本
      │          └─ 无按钮：从 DOM 提取 innerText
      └─ 未找到：向下滚动 → 检查是否已经稳定到达底部
  → 恢复剪贴板方法
  → 导出 conversation 变量
```

ChatGPT 使用虚拟列表，页面中通常只挂载可视区域附近的消息。因此这里不能使用一次性的“遍历元素”节点，而是维护已处理 ID 集合并逐步向下滚动。只有同时满足以下条件才结束：

- 距离滚动容器底部不超过 `120px`；
- 当前挂载节点中没有未处理的 turn；
- 上述空闲状态连续出现至少三轮。

连续三轮确认用于等待滚动后异步挂载的新消息，避免刚到页面底部就提前导出。

## 关键节点

| 节点 ID | Automa 节点 | 作用 |
| --- | --- | --- |
| `uu3mmo8` | JavaScript：校验页面 | 查找 user、assistant 和全部 turn；完全找不到时中止 |
| `xxep41u` | JavaScript：拦截剪贴板 | 代理 `writeText`/`write`，用递增序号区分每次复制 |
| `jiglnit` | JavaScript：初始化数组 | 清空导出数组、当前 turn 变量、已处理集合和空闲轮数 |
| `7qbifjr` | While loop | 在 `chatgptDone` 变为真之前持续扫描 |
| `4xsplhe` | JavaScript：Find Next Turn | 按 `conversation-turn-N` 排序，选择第一个未处理的 `data-turn-id` |
| `yzpfobl` | Conditions：Has Turn | 在处理消息和继续滚动之间分流 |
| `itnk3lc` | Hover element | 让当前 turn 的操作栏显示出来 |
| `g0nvccq` | Conditions：Has Copy | 判断当前 turn 是否存在复制按钮 |
| `1h4zphk` | Click element | 点击 ChatGPT 的 turn 复制按钮 |
| `b0vvwkb` | JavaScript：Consume clipboard | 校验本轮产生了新的复制结果并追加消息 |
| `p9m4ly0` | JavaScript：DOM fallback | 没有复制按钮时从消息 DOM 提取文本 |
| `oo4xuuw` | JavaScript：Scroll Down | 每轮向下滚动 `max(0.75 × 可视高度, 500px)` |
| `m4xgcts` | JavaScript：Check Done | 检查底部、未处理节点和连续空闲轮数 |
| `3cnis15` | JavaScript：恢复剪贴板 | 正常结束时还原页面原有 clipboard 方法 |
| `ddyc51w` | Export data | 将 `conversation` 变量导出为 JSON |

## 页面选择器

| 用途 | 当前选择器或属性 | 调整时注意 |
| --- | --- | --- |
| 消息容器 | `section[data-testid^="conversation-turn-"][data-turn][data-turn-id]` | 三个属性分别承担排序/定位、角色和稳定去重；不能只检查元素文本 |
| 用户角色 | `data-turn="user"` | 输出必须规范化为 `user` |
| 助手角色 | `data-turn="assistant"` | 输出必须规范化为 `assistant` |
| turn 顺序 | `data-testid="conversation-turn-N"` 中的数字 | DOM 顺序受虚拟列表影响，因此显式解析数字排序 |
| turn 身份 | `data-turn-id` | 用于 `window.__chatgptSeen` 去重，也用于构造唯一选择器 |
| 复制按钮 | `button[data-testid="copy-turn-action-button"]` | 按钮通常要悬停后才可见 |
| 消息正文 fallback | `[data-message-author-role="${role}"]` | 优先取正文，避免把操作栏文字混进内容 |
| 滚动容器 | `[data-scroll-root]` | 若 ChatGPT 改变滚动层级，滚动和完成判断必须同时更新 |

## 状态与输出

Automa 变量：

- `conversation`：最终消息数组；
- `currentTurnSelector`、`currentTurnId`、`currentTurnRole`、`currentTurnNumber`：当前消息上下文；
- `currentTurnFound`：控制“处理消息/继续滚动”分支；
- `chatgptDone`：控制 while loop；
- `chatgptPageCheck`：启动时观察到的页面统计，只用于诊断。

页面内临时状态：

- `window.__chatgptSeen`：已经追加到结果的 `data-turn-id`；
- `window.__chatgptIdleRounds`：没有发现新消息的连续轮数；
- `window.__chatgptCopiedText`、`__chatgptCopySeq`、`__chatgptLastReadSeq`：剪贴板捕获值及消费序号。

正常复制得到的对象结构为：

```json
{
  "role": "assistant",
  "content": "消息 Markdown",
  "turnId": "来源 turn ID",
  "turnNumber": 2
}
```

DOM fallback 会额外写入 `"copied": false`。Palace 当前只消费 `role` 和 `content`，其余字段用于排障和核对来源顺序；修改工作流时不要改变这两个核心字段的语义。

## 时序与失败处理

- 悬停后等待 `200ms`，让操作按钮完成渲染。
- 点击复制后等待 `300ms`，让异步 clipboard 调用完成。
- 每次滚动后等待 `500ms`，让虚拟列表完成卸载和挂载。
- `Consume clipboard` 要求复制序号严格增加，防止把上一条消息重复追加。
- 工作流的全局错误策略是停止执行。若在恢复节点之前失败，clipboard 代理可能继续留在当前页面；刷新页面即可恢复，再从顶部重新运行。

## 站点改版后的调整顺序

1. 在浏览器控制台确认消息容器、角色、ID、复制按钮和滚动根节点是否仍能命中。
2. 若只改了 DOM 属性，优先同步“校验页面”“Find Next Turn”“Has Copy”“DOM fallback”“Scroll Down”和“Check Done”中的全部相关选择器，避免只修一处。
3. 若复制按钮不再调用 Clipboard API，先确认新的复制链路，再决定拦截新接口或直接从稳定正文 DOM 提取；不要静默降级到整个 turn 的 `innerText`。
4. 用至少包含 user、assistant、代码块、长回复和足以触发虚拟滚动的对话回归。
5. 检查导出数组无重复、顺序正确、首尾完整，随后用 Palace 实际导入一次。
6. 在 Automa 中重新导出后，用 `jq empty export-conversation.automa.json` 检查 JSON 完整性，并同步更新本文中的节点、选择器和时序说明。
