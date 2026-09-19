# Gemini 对话导出工作流维护说明

本目录的 [`export-conversation.automa.json`](export-conversation.automa.json) 是 Automa `1.30.02` 导出的工作流。它使用 Automa 原生页面节点遍历当前 Gemini 对话，通过系统剪贴板取得每条消息的纯文本，最后下载 `gemini-conversation.json`。

Gemini 的 Trusted Types 策略会阻止 Automa `JavaScript Code / Active Tab` 节点通过 `<script>.textContent` 注入代码。因此本工作流不包含 Active Tab JavaScript；页面查找、悬停、点击和滚动全部由 Automa 原生节点执行，JavaScript 只在 Background 上下文中整理工作流变量。

## 使用前提

1. 在 Automa 中导入工作流 JSON。
2. 向 Automa 授予读取剪贴板权限；没有该权限时 `Clipboard` 节点会中止。
3. 打开目标 Gemini 对话，等待回答生成完毕。
4. 将对话滚动到顶部后手动运行工作流，从中间启动可能漏掉更早的虚拟列表节点。
5. 运行期间不要手动复制其他内容，也不要切换或滚动当前 Gemini 页面。
6. 导出后检查首尾消息、消息数量、公式和代码块，再把 JSON 导入 Palace。

工作流会清空并使用系统剪贴板，结束后保留最后一条 Gemini 消息。清空操作用于避免复制失败时错误复用上一条消息。

## 总体流程

```text
手动触发
  → 绑定当前标签页
  → Background JavaScript 初始化 conversation
  → Loop elements 遍历当前挂载消息并逐段向下滚动
      → 清空系统剪贴板
      → 悬停当前消息
      → 等待操作按钮渲染
      → 根据复制按钮识别 user / assistant
      → Background JavaScript 记录角色
      → 原生 Click element 点击对应复制按钮
      → 等待 500ms
      → Clipboard 读取复制结果
      → Background JavaScript 追加 { role, content }
      → 检查是否连续两次遇到导出列表的第 1 或第 2 条消息
          ├─ 是：删除两条回卷副本并直接导出
          └─ 否：继续当前批次或加载下一批
  → 导出 conversation 变量
```

`Loop elements` 会给已经发现的 DOM 节点添加 Automa 内部标记。当前批次处理完后，它把最后一个已标记节点滚入视口，等待 `500ms`，再查找新挂载且未标记的消息。这避免了一次性读取当前 DOM，也不会直接跳到列表底部。

消息集合必须使用 `:is(user-query, model-response)`。Automa 1.30 查找下一批消息时，会直接在整个选择器末尾追加 `:not([automa-loop*="…"])`。如果写成 `user-query, model-response`，排除条件只作用于 `model-response`，已经处理的所有 `user-query` 仍会被再次选中。第一批复制完成后，下一批因此只包含 user；处理首条 user 时页面回到顶部，形成“回卷后只复制 user”的循环。这个问题在静态 DOM 上也能复现，不需要虚拟列表卸载节点。

使用 `:is(...)` 后，排除条件同时作用于 user 和 assistant，无新节点时原生循环会正常结束。虚拟列表真正卸载、重新挂载节点时，Automa 的标记仍可能丢失；因此后台保留内容回卷检测作为兜底：保存最初导出的两条消息，连续两条新消息都命中任一锚点时，删除这两条副本并直接导出。该兜底不是可靠的节点身份判断，也不能保证识别所有部分回放。

单次合法重复第一个或第二个锚点不会停止；下一条正常消息会把连续命中计数清零，并保留前一条合法重复。只有一问一答时，第一条用户消息连续被复制两次就会确认回卷并退出。

确认回卷后的分支直接连接 Export data，不经过 `Loop breakpoint`。Automa 1.30 的元素循环即使在 breakpoint 上设置 `clearLoop: true`，仍会先执行 `loadMoreAction`；只要发现新匹配的节点，就可能重新进入循环。因此不能依赖该选项强制退出带滚动加载的元素循环。

## 关键节点

| 节点 ID | Automa 节点 | 作用 |
| --- | --- | --- |
| `jiglnit` | JavaScript / Background | 初始化 `conversation`、当前角色和当前复制文本 |
| `zzmi7il` | Loop elements | 遍历 `:is(user-query, model-response)`，以 `scroll` 方式逐段加载 |
| `clrclip` | Clipboard / Insert | 在处理每条消息前清空系统剪贴板 |
| `j5a7mlm` | Hover element | 让当前消息的操作按钮显示出来 |
| `hoverwait` | Delay | 等待操作按钮完成渲染 |
| `xvm5a4f` | Conditions | 根据当前容器中的复制按钮识别角色 |
| `setuser`、`setasst` | JavaScript / Background | 将当前角色写入工作流变量 |
| `rxdvjyj` | Click element | 点击用户问题的复制按钮 |
| `asstcopy` | Click element | 点击助手回答的复制按钮 |
| `mvrr91u` | Delay | 等待复制动作写入系统剪贴板 |
| `readclip` | Clipboard / Get | 将剪贴板文本写入 `currentCopiedText` |
| `i6mlj2r` | JavaScript / Background | 校验并追加消息，保存前两个锚点，连续命中两次时删除两条回卷副本 |
| `loopdone` | Conditions | 确认回卷时直接进入导出，否则继续循环 |
| `cqwhcdl` | Loop breakpoint | 继续当前批次，或触发下一段滚动和加载 |
| `qrwsswr` | Export data | 将 `conversation` 变量导出为 JSON |

## 页面选择器

| 用途 | 当前选择器 | 调整时注意 |
| --- | --- | --- |
| 消息集合 | `:is(user-query, model-response)` | 必须保留 `:is(...)`，让 Automa 追加的排除条件同时作用于两种消息 |
| 用户复制按钮 | `button:is([aria-label="Copy prompt"], [aria-label="复制提示"])` | 查询限定在当前 loop selector 中 |
| 助手复制按钮 | `button:is([aria-label="Copy"], [aria-label="复制回答"])` | 查询限定在当前 loop selector 中 |

角色由 Conditions 中的按钮选择器决定，不使用位置奇偶或消息文本推断。若 Gemini 修改 aria-label，必须同时更新 Conditions 和相应 Click element。

## 状态与输出

Automa 变量：

- `conversation`：最终消息数组；
- `currentMessageRole`：当前消息的 `user` 或 `assistant` 角色；
- `currentCopiedText`：`Clipboard` 节点读取到的本轮复制内容；
- `replayAnchors`：最初导出的两条消息，按完整的 `role` 和 `content` 保存；
- `replayAnchorRun`：连续命中两个回卷锚点中任意一个的次数；
- `geminiLoopDone`：是否已经确认虚拟列表回卷。

每条消息输出为：

```json
{
  "role": "user",
  "content": "消息 Markdown"
}
```

`role` 只能是 `user` 或 `assistant`，`content` 必须为非空字符串。页面没有成功复制时，前置清空使 `currentCopiedText` 保持为空，后台消费节点会中止工作流，而不会静默复用旧内容。

## 时序与虚拟列表边界

- 悬停后等待 `200ms`，让操作按钮完成渲染。
- 点击复制后等待 `500ms`，随后读取系统剪贴板。
- `Loop elements` 的加载动作为 `scroll`，`scrollToBottom` 为 `false`，不会一次跳到页面底部。
- `maxLoop` 为 `2000`，即使页面结构异常且回卷检测没有命中，也不会无限执行。
- Automa 每次滚动后等待 `500ms` 再寻找新节点；如果 Gemini 挂载消息更慢，原生循环可能提前结束。
- Automa 使用 DOM 属性标记已处理节点。如果 Gemini 改为原地复用同一个 DOM 节点承载另一条消息，新内容可能继承旧标记而被跳过。
- 回卷判断同时比较角色和完整正文；连续两条消息都命中第一个或第二个锚点中的任意一个才会触发退出。
- 单次合法重复锚点会保留；如果后续对话合法地连续重复了两个锚点消息，仍会被误判为回卷。这是无法读取页面稳定消息 ID 时的剩余歧义。
- 当前原生节点方案无法使用页面稳定 ID 和底部连续三轮确认；这是为绕开 Gemini Trusted Types 与 Automa Active Tab JavaScript 不兼容而接受的限制。

## 失败处理与排障

- 工作流设置保持普通模式；开启 Debug mode 不能修复 Gemini 对 Active Tab JavaScript 的 Trusted Types 拦截。
- 若看到 `no-clipboard-acces`，在 Automa 中授予 Clipboard 权限后重试。
- 若在第一条消息处找不到复制按钮，检查 Gemini 当前语言以及两类 aria-label。
- 若导出在中途结束，先观察是否确实逐段滚动；必要时增加 Loop elements 的等待时间。
- 若仍然回到顶部，检查后台日志是否出现 `[Gemini export] replay anchors confirmed`，且其中 `replayAnchorRun` 为 `2`、`done` 为 `true`。
- 若第二轮只复制 user，首先检查导入的 Loop elements 选择器是否为 `:is(user-query, model-response)`；裸逗号列表会使已处理 user 被再次选中。
- 若导出内容缺失，检查后续消息是否合法连续重复了最初两个锚点中的消息，或 Gemini 是否复用了带 `automa-loop` 属性的消息节点。
- 工作流的全局错误策略是停止执行，避免生成内容与角色错位但看似成功的文件。

## 站点改版后的调整顺序

1. 在浏览器控制台检查 `user-query`、`model-response` 和两类复制按钮是否仍存在。
2. 若消息容器改名，同步修改 Loop elements 的消息集合，并保留外层 `:is(...)`。
3. 若 aria-label 改变，同步修改 Conditions 和对应的 Click element。
4. 用包含连续消息、代码块、列表、公式和足以触发虚拟滚动的长对话验证顺序、角色、首尾完整性及无重复。
5. 检查系统剪贴板权限和每次点击后的实际复制结果。
6. 在 Automa 中重新导出后，用 `jq empty export-conversation.automa.json` 检查 JSON 完整性，并同步更新本文。

## 本地回归验证

安装项目 npm 依赖后，在仓库根目录运行 `node --test examples/gemini/export-conversation.test.mjs`。测试读取实际工作流选择器，使用 jsdom 验证首轮顺序、全部标记后无第二批 user、后续加载只返回新消息，以及回卷确认分支直接到达 Export。它不代替 Gemini 登录页面上的完整运行验证。
