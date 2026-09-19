# Grok 对话导出工作流维护说明

本目录的 [`export-conversation.automa.json`](export-conversation.automa.json) 是 Automa `1.30.02` 导出的工作流。它遍历当前 Grok 对话的消息锚点，调用每条消息的复制操作取得纯文本，最后下载 `grok-conversation.json`。同名文件已存在时，Automa 会自动生成不冲突的文件名。

## 使用前提

1. 在 Automa 中导入工作流 JSON。
2. 打开目标 Grok 对话，等待回答生成完毕。
3. 建议先滚动到对话顶部，再手动运行工作流；这样更容易覆盖页面懒加载或虚拟化管理的早期消息。
4. 导出后检查首尾消息、消息数量、代码块和引用内容，再把 JSON 导入 Palace。

工作流不会真正写入系统剪贴板。它代理页面中的 `navigator.clipboard.writeText` 和 `navigator.clipboard.write`，捕获 Grok 复制按钮准备写入的 `text/plain`。

## 总体流程

```text
手动触发
  → 绑定当前标签页
  → 校验 Grok user/assistant 消息
  → 安装剪贴板拦截器
  → 初始化 conversation
  → 按 DOM 顺序遍历 response-* 消息锚点
      → 悬停当前锚点
      → 根据内部 data-testid 判断角色
      → 点击对应复制按钮
      → 等待 clipboard 回调
      → 再次校验角色并追加 { role, content }
  → 结束遍历
  → 导出 conversation 变量
```

Grok 的用户和助手消息外层都可能使用 `id="response-*"`，因此不能根据 ID 前缀判断角色。工作流只用外层锚点确定遍历范围，再根据其内部的 `data-testid="user-message"` 或 `data-testid="assistant-message"` 识别角色。

## 关键节点

| 节点 ID | Automa 节点 | 作用 |
| --- | --- | --- |
| `uu3mmo8` | JavaScript：校验页面 | 统计 user/assistant 测试节点，两者都不存在时中止 |
| `xxep41u` | JavaScript：拦截剪贴板 | 代理 `writeText`/`write` 并记录最新纯文本和序号 |
| `jiglnit` | JavaScript：初始化数组 | 清空 `conversation` 和 `currentMessage` |
| `zzmi7il` | Loop elements | 遍历全部 `response-*` 消息锚点，并开启滚动到底部 |
| `j5a7mlm` | Hover element | 让当前消息的操作按钮显示出来 |
| `xvm5a4f` | Conditions | 按内部 `data-testid` 分为 User 和 Assistant |
| `rxdvjyj` | Click element | 点击用户消息的 `Copy` 按钮 |
| `0wxah8h` | JavaScript | 定位并点击助手消息的 `Copy response` 按钮 |
| `mvrr91u` | Delay | 等待复制动作进入 clipboard 代理 |
| `i6mlj2r` | JavaScript | 校验新复制序号、识别角色并追加结果 |
| `cqwhcdl` | Loop breakpoint | 结束 loop `PZypES` 的当前处理链 |
| `qrwsswr` | Export data | 将 `conversation` 变量导出为 JSON |

## 页面选择器

| 用途 | 当前选择器 | 调整时注意 |
| --- | --- | --- |
| 消息遍历锚点 | `[data-scroll-anchor-root="true"][id^="response-"]` | ID 只负责定位消息容器，不能用于判断角色 |
| 用户消息标记 | `[data-testid="user-message"]` | Conditions 和消费节点各检查一次 |
| 助手消息标记 | `[data-testid="assistant-message"]` | Conditions 和消费节点各检查一次 |
| 用户复制按钮 | `button[aria-label="Copy"]` | 查询被限定在当前 loop selector 内 |
| 助手复制按钮 | `button[aria-label="Copy response"]` | 与用户按钮不同，由 JavaScript 主动点击 |

`{{loopData@PZypES}}` 是 Automa 为当前迭代元素生成的临时 CSS selector。按钮和角色查询都必须限定在这个 selector 下，否则可能读取或点击另一条消息。

## 状态与输出

Automa 变量：

- `conversation`：最终消息数组；
- `currentMessage`：最近追加的消息，用于运行日志和排障；
- `grokPageCheck`：启动时观察到的角色数量与 URL。

页面内临时状态：

- `window.__grokCopiedText`：最近捕获的纯文本；
- `window.__grokCopySeq`：每次成功捕获后递增；
- `window.__grokLastReadSeq`：最近已经追加到数组的序号；
- `window.__grokClipboardInstalled`：避免同一页面重复包装 clipboard 方法。

输出中的每条消息固定为：

```json
{
  "role": "assistant",
  "content": "消息 Markdown"
}
```

`role` 只能是 `user` 或 `assistant`，`content` 必须来自当前消息触发的新复制事件。递增序号用于阻止复制失败后复用上一条消息内容。

## 时序与已知边界

- 点击复制后固定等待 `300ms`；如果页面复制动作变慢，应根据控制台日志调整，而不是任意增加长延迟。
- Loop elements 的 `maxLoop` 当前为 `0`，工作流按“不人为限制导出条数”的预期使用；升级 Automa 后应确认该值的语义没有变化。
- 工作流的全局错误策略是停止执行。缺少按钮、角色不明确或没有新的 clipboard 事件都会直接失败，以免生成内容与角色错位的文件。
- 当前工作流不会在结束时还原 clipboard 方法。刷新 Grok 页面可以恢复原方法；若新增恢复节点，需要保存原始方法并覆盖正常结束和异常中止场景。
- 如果 Grok 开始大量卸载屏幕外消息，Loop elements 可能漏掉未挂载节点；此时应改为“已处理集合 + 分段滚动 + 多轮稳定确认”的状态循环。

## 站点改版后的调整顺序

1. 在浏览器控制台检查消息锚点、两类 `data-testid` 和复制按钮是否仍能命中。
2. 外层 `response-*` 命名变化时，只调整遍历锚点；角色仍应来自明确的 user/assistant 标记，不能用位置奇偶或按钮文字推断。
3. 角色标记变化时，同步修改 Conditions 和消费 JavaScript 中的两套判断。
4. 按钮 aria-label 变化时，同步修改用户 Click 节点和助手 JavaScript 节点。
5. 若复制不再经过 Clipboard API，优先寻找 Grok 自己生成的 Markdown/纯文本结果，避免直接抓取包含操作栏的整个容器文本。
6. 用包含 user、assistant、代码块、引用、长回复和长对话的样本验证顺序、角色、首尾完整性及无重复。
7. 在 Automa 中重新导出后，用 `jq empty export-conversation.automa.json` 检查 JSON 完整性，并同步更新本文。
