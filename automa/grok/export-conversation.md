# Grok 对话导出工作流维护说明

本目录的 [`export-conversation.automa.json`](export-conversation.automa.json) 是 Automa `1.30.02` 导出的工作流。它在当前 Grok 对话页逐条触发复制操作，将结果整理为 JSON 并下载为 `grok-conversation.json`。同名文件已存在时，Automa 会自动生成不冲突的文件名。

## 使用前提

1. 在 Automa 中导入工作流 JSON。
2. 打开目标 Grok 对话，等待回答生成完毕。
3. 将对话滚动到顶部后手动运行工作流。工作流只会从当前已经挂载的消息开始向下扫描，从中间启动可能漏掉更早的虚拟列表节点。
4. 导出后检查首尾消息、消息数量、代码块和引用内容，再把 JSON 导入 Palace。

工作流不会真正写入系统剪贴板。它会临时代理页面中的 `navigator.clipboard.writeText` 和 `navigator.clipboard.write`，捕获 Grok 复制按钮准备写入的 `text/plain`。

## 总体流程

```text
手动触发
  → 绑定当前标签页
  → 校验 Grok 消息节点
  → 安装剪贴板拦截器
  → 初始化变量和页面内状态
  → 循环查找未处理的消息
      ├─ 找到：悬停 → 按角色点击复制按钮 → 捕获复制文本
      └─ 未找到：向下滚动 → 检查是否已经稳定到达底部
  → 恢复剪贴板方法
  → 导出 conversation 变量
```

Grok 可能使用虚拟列表，页面中不保证同时挂载整段对话。因此工作流不再一次性遍历 `response-*` 节点，而是维护已处理消息集合并逐步向下滚动。只有同时满足以下条件才结束：

- 距离滚动容器底部不超过 `120px`；
- 当前挂载节点中没有未处理的消息；
- 上述空闲状态连续出现至少三轮。

连续三轮确认用于等待滚动后的异步挂载，避免刚到页面底部就提前导出。

## 关键节点

| 节点 ID | Automa 节点 | 作用 |
| --- | --- | --- |
| `uu3mmo8` | JavaScript：校验页面 | 查找 user/assistant 消息；完全找不到时中止 |
| `xxep41u` | JavaScript：拦截剪贴板 | 代理 `writeText`/`write`，用递增序号区分每次复制 |
| `jiglnit` | JavaScript：初始化状态 | 清空导出数组、当前消息变量、已处理集合和空闲轮数 |
| `7qbifjr` | While loop | 在 `grokDone` 变为真之前持续扫描 |
| `4xsplhe` | JavaScript：Find Next Message | 按 DOM 顺序选择第一个尚未处理的挂载消息 |
| `yzpfobl` | Conditions：Has Message | 在处理消息和继续滚动之间分流 |
| `itnk3lc` | Hover element | 让当前消息的操作按钮显示出来 |
| `1h4zphk` | JavaScript：Click Copy | 根据当前角色定位并点击用户或助手复制按钮 |
| `b0vvwkb` | JavaScript：Consume clipboard | 校验本轮产生了新的复制结果并追加消息 |
| `oo4xuuw` | JavaScript：Scroll Down | 每轮向下滚动 `max(0.75 × 可视高度, 500px)` |
| `m4xgcts` | JavaScript：Check Done | 检查底部、未处理节点和连续空闲轮数 |
| `3cnis15` | JavaScript：恢复剪贴板 | 正常结束时还原页面原有 clipboard 方法 |
| `ddyc51w` | Export data | 将 `conversation` 变量导出为 JSON |

## 页面选择器与消息身份

| 用途 | 当前选择器或属性 | 调整时注意 |
| --- | --- | --- |
| 消息锚点 | `[data-scroll-anchor-root="true"][id^="response-"]` | `response-*` ID 用于稳定去重和定位，不用于判断角色 |
| 用户消息标记 | `[data-testid="user-message"]` | 在当前消息锚点内部识别角色 |
| 助手消息标记 | `[data-testid="assistant-message"]` | 在当前消息锚点内部识别角色 |
| 用户复制按钮 | `button[aria-label="Copy"]` | 必须限定在当前消息锚点中查询 |
| 助手复制按钮 | `button[aria-label="Copy response"]` | 与用户按钮不同，按当前角色选择 |
| 滚动容器 | 当前消息向上的第一个可纵向滚动祖先；否则使用 `document.scrollingElement` | 滚动和完成判断共用同一套查找规则 |

Grok 的用户和助手消息外层都可能使用 `id="response-*"`，因此角色必须来自内部 `data-testid`。工作流优先使用外层 `id` 作为稳定身份，并把完整身份写入 `data-palace-export-key` 供当前轮定位。通用的备用身份逻辑也会检查消息 ID、序号属性及文本变化，但 `response-*` 仍是当前首选来源。

## 状态与输出

Automa 变量：

- `conversation`：最终消息数组；
- `currentMessageSelector`、`currentMessageId`、`currentMessageRole`：当前消息上下文；
- `currentMessageFound`：控制“处理消息/继续滚动”分支；
- `grokDone`：控制 while loop；
- `grokPageCheck`：启动时观察到的页面统计，只用于诊断。

页面内临时状态：

- `window.__grokSeen`：已经追加到结果的消息身份；
- `window.__grokIdleRounds`：没有发现新消息的连续轮数；
- `window.__grokElementKeys`、`window.__grokMountedKeySeq`：备用运行期身份状态；
- `window.__grokCopiedText`、`__grokCopySeq`、`__grokLastReadSeq`：剪贴板捕获值及消费序号。

每条消息输出为：

```json
{
  "role": "assistant",
  "content": "消息 Markdown",
  "messageId": "assistant:response-..."
}
```

Palace 当前只消费 `role` 和 `content`，`messageId` 用于排障和核对去重；修改工作流时不要改变两个核心字段的语义。

## 时序与失败处理

- 悬停后等待 `200ms`，让操作按钮完成渲染。
- 点击复制后等待 `300ms`，让异步 clipboard 调用完成。
- 每次滚动后等待 `500ms`，让虚拟列表完成卸载和挂载。
- `Consume clipboard` 要求复制序号严格增加，防止把上一条消息重复追加。
- 工作流的全局错误策略是停止执行。找不到按钮、角色无法识别或本轮没有新复制内容都会中止。
- 若在恢复节点之前失败，clipboard 代理可能继续留在当前页面；刷新页面即可恢复，再从顶部重新运行。

## 站点改版后的调整顺序

1. 在浏览器控制台确认消息锚点、稳定 ID、两类 `data-testid`、复制按钮和实际滚动容器是否仍能命中。
2. 外层消息锚点变化时，同步修改“校验页面”“Find Next Message”“Scroll Down”和“Check Done”。
3. 角色标记变化时，同步修改查找和完成判断中的角色识别；不能用位置奇偶或按钮文字猜测角色。
4. 按钮 aria-label 变化时，同步修改 `Click Copy` 中对应角色的按钮选择器。
5. 若复制不再经过 Clipboard API，优先寻找 Grok 自己生成的 Markdown/纯文本结果，避免直接抓取包含操作栏的整个容器文本。
6. 用至少包含 user、assistant、代码块、引用、长回复和足以触发虚拟滚动的对话回归。
7. 检查导出数组无重复、顺序正确、首尾完整，随后用 Palace 实际导入一次。
8. 在 Automa 中重新导出后，用 `jq empty export-conversation.automa.json` 检查 JSON 完整性，并同步更新本文中的节点、选择器和时序说明。
