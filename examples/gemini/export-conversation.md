# Gemini 对话导出工作流维护说明

本目录的 [`export-conversation.automa.json`](export-conversation.automa.json) 是 Automa `1.30.02` 导出的工作流。它遍历当前 Gemini 对话中的用户问题和模型回答，通过页面复制按钮取得纯文本，最后下载 `gemini-conversation.json`。同名文件已存在时，Automa 会自动生成不冲突的文件名。

## 使用前提

1. 在 Automa 中导入工作流 JSON。
2. 打开目标 Gemini 对话，等待回答生成完毕。
3. 建议先滚动到对话顶部，再手动运行工作流；这样更容易覆盖由页面懒加载或虚拟化管理的早期消息。
4. 导出后检查首尾消息、消息数量、公式和代码块，再把 JSON 导入 Palace。

工作流不会真正覆盖系统剪贴板。它代理页面中的 `navigator.clipboard.writeText` 和 `navigator.clipboard.write`，捕获 Gemini 复制按钮准备写入的 `text/plain`。

## 总体流程

```text
手动触发
  → 绑定当前标签页
  → 校验 Gemini 消息节点
  → 安装剪贴板拦截器
  → 初始化 conversation
  → 按 DOM 顺序遍历 user-query, model-response
      → 悬停当前元素
      → 根据复制按钮判断 user / assistant
      → 点击对应复制按钮
      → 等待 clipboard 回调
      → 识别角色并追加 { role, content }
  → 结束遍历
  → 导出 conversation 变量
```

角色分支不是根据文本或消息位置猜测，而是检查当前元素中存在的复制按钮：用户按钮为 `Copy prompt`，助手按钮为 `Copy`。消费复制结果时会再次根据自定义元素 `user-query` / `model-response` 校验角色，避免分支与实际容器不一致。

## 关键节点

| 节点 ID | Automa 节点 | 作用 |
| --- | --- | --- |
| `uu3mmo8` | JavaScript：校验页面 | 统计 `user-query` 和 `model-response`，两者都不存在时中止 |
| `xxep41u` | JavaScript：拦截剪贴板 | 代理 `writeText`/`write` 并用递增序号保存最新纯文本 |
| `jiglnit` | JavaScript：初始化数组 | 清空 `conversation` 和 `currentMessage` |
| `zzmi7il` | Loop elements | 按页面顺序遍历全部消息容器，并开启滚动到底部 |
| `j5a7mlm` | Hover element | 让当前消息的操作按钮显示出来 |
| `xvm5a4f` | Conditions | 根据当前容器中的复制按钮分为 User 和 Assistant |
| `rxdvjyj` | Click element | 点击用户问题的复制按钮 |
| `0wxah8h` | JavaScript | 定位并点击助手回答的复制按钮 |
| `mvrr91u` | Delay | 等待复制动作进入 clipboard 代理 |
| `i6mlj2r` | JavaScript | 校验新复制序号、识别角色并追加结果 |
| `cqwhcdl` | Loop breakpoint | 结束 loop `PZypES` 的当前处理链 |
| `qrwsswr` | Export data | 将 `conversation` 变量导出为 JSON |

## 页面选择器

| 用途 | 当前选择器 | 调整时注意 |
| --- | --- | --- |
| 用户消息容器 | `user-query` | 同时用于遍历和最终角色校验 |
| 助手消息容器 | `model-response` | 同时用于遍历和最终角色校验 |
| 遍历集合 | `user-query, model-response` | CSS 选择器返回 DOM 顺序，决定导出顺序 |
| 用户复制按钮 | `button[aria-label="Copy prompt"], button[aria-label="复制提示"]` | 当前兼容英文和中文界面；新增语言时必须同步条件节点与点击节点 |
| 助手复制按钮 | `button[aria-label="Copy"], button[aria-label="复制回答"]` | 条件节点与 JavaScript 点击逻辑必须保持一致 |

`{{loopData@PZypES}}` 是 Automa 为当前迭代元素提供的临时 CSS selector。所有当前元素查询都必须限定在这个 selector 下，否则容易点击到页面中第一条消息的按钮。

## 状态与输出

Automa 变量：

- `conversation`：最终消息数组；
- `currentMessage`：最近追加的消息，主要用于运行日志和排障；
- `geminiPageCheck`：启动时的 user/assistant 数量与 URL。

页面内临时状态：

- `window.__geminiCopiedText`：最近捕获的纯文本；
- `window.__geminiCopySeq`：每次成功捕获后递增；
- `window.__geminiLastReadSeq`：最近已经追加到数组的序号；
- `window.__geminiClipboardInstalled`：避免同一页面重复包装 clipboard 方法。

输出中的每条消息固定为：

```json
{
  "role": "user",
  "content": "消息 Markdown"
}
```

`role` 只能是 `user` 或 `assistant`，`content` 必须来自本轮新的复制事件。序号校验用于防止复制失败时错误复用上一条消息内容。

## 时序与已知边界

- 点击复制后固定等待 `300ms`；如果页面改为更慢的异步生成，可适当增加，但应先用日志确认真实边界。
- Loop elements 的 `maxLoop` 当前为 `0`，工作流按“不人为限制导出条数”的预期使用；升级 Automa 后应确认该值的语义没有变化。
- 工作流的全局错误策略是停止执行，找不到按钮、角色无法识别或本轮没有新复制内容都会中止，避免导出看似成功但内容错位的文件。
- 当前工作流不会在结束时还原 clipboard 方法。刷新 Gemini 页面可以恢复原方法；若新增恢复节点，需要像 ChatGPT 工作流一样保存原始 `writeText`/`write`，并考虑错误中止路径。
- 页面懒加载行为改变后，单纯的 Loop elements 可能无法覆盖未挂载消息；此时应改成类似 ChatGPT 的“已处理集合 + 分段滚动 + 稳定结束”状态循环。

## 站点改版后的调整顺序

1. 在浏览器控制台检查 `user-query`、`model-response` 和两类复制按钮是否仍存在。
2. 若 aria-label 改变，同步修改 Conditions 中的角色判断、用户 Click 节点和助手 JavaScript 节点，不能只改其中一处。
3. 若消息容器改名，同步修改页面校验、Loop elements 和消费节点中的角色识别。
4. 若复制不再经过 Clipboard API，确认新的数据来源后再改捕获策略；优先复用网站自己的复制结果，以保留 Markdown、代码块和公式格式。
5. 用包含连续消息、代码块、列表、公式和长对话的样本验证顺序、角色、首尾完整性及无重复。
6. 在 Automa 中重新导出后，用 `jq empty export-conversation.automa.json` 检查 JSON 完整性，并同步更新本文。
