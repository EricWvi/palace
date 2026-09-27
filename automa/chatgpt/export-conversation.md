# ChatGPT 对话导出工作流维护说明

本目录的 [`export-conversation.automa.json`](export-conversation.automa.json) 是 Automa `1.30.02` 导出的工作流。它使用 Automa 原生页面节点遍历当前 ChatGPT 对话，通过系统剪贴板取得每条消息的复制结果，最后下载 `chatgpt-conversation.json`。同名文件已存在时，Automa 会自动生成不冲突的文件名。

ChatGPT 的 CSP 只允许带 nonce 或指定 hash 的内联脚本。Automa `JavaScript Code / Active Tab` 节点以内联 `<script>` 注入代码，因此会被拦截，控制台报错 `Executing inline script violates the following Content Security Policy directive 'script-src-elem …'`。本工作流因此不包含 Active Tab JavaScript：页面读取、滚动、悬停和点击全部由 Automa 原生节点在扩展的 content script 中执行，JavaScript 只在 Background 上下文中整理工作流变量。做法与 [Gemini 工作流](../gemini/export-conversation.md) 相同。

## 使用前提

1. 在 Automa 中导入工作流 JSON。
2. 向 Automa 授予读取剪贴板权限；没有该权限时 `Clipboard` 节点会中止。
3. 打开需要导出的 ChatGPT 对话，等待回复生成结束。
4. 手动运行工作流。工作流会先自动滚到对话顶部，无需手动滚动。
5. 运行期间不要手动复制其他内容，也不要切换或滚动当前 ChatGPT 页面。
6. 导出后检查首尾消息、消息数量以及包含代码块的消息，再把 JSON 导入 Palace。

工作流会真实写入系统剪贴板：每次点击复制前先写入占位文本，结束后保留最后一条消息。占位文本用于识别复制失败，避免错误复用上一条消息；不使用空字符串，因为 `execCommand("copy")` 在没有选中内容时可能不改写剪贴板。

## 页面结构

2026 年 9 月的 ChatGPT 页面不再提供 `data-testid="conversation-turn-N"`、`data-turn`、`data-turn-id`、`data-message-author-role` 和 `data-scroll-root`。当前结构以“问答轮次”为单位：

```text
div[data-turn-key="<user 消息 ID>"]                      ← 一轮问答，循环单位
  └─ div.group.flex.flex-col
       ├─ div.flex.flex-col
       │    ├─ div[data-chatgpt-search-unit-key="…:user"]
       │    │    └─ .turn-action-controls button[aria-label="复制消息"]   ← user 复制
       │    └─ div[data-chatgpt-search-unit-key="…:assistant"]
       │         └─ 代码块工具栏 button[aria-label="复制"]               ← 不能点
       └─ .turn-action-controls button[aria-label="复制"]               ← assistant 复制
```

assistant 的复制按钮位于整轮底部的操作栏，不在 assistant 单元内部；代码块工具栏也有 `aria-label="复制"` 的按钮。因此 assistant 选择器必须限定在 `.turn-action-controls` 下。

滚动容器为 `.thread-scroll-container`，其 `flex-direction` 为 `column-reverse`：`scrollTop` 为 `0` 时位于对话底部，越往上越负。`scrollTo(0, 0)` 因此回到底部而不是顶部；滚到顶部需要设置足够大的负值，浏览器会把它截到最小值。

对话是虚拟列表，只挂载视口附近的若干轮。2026 年 9 月在 9 轮对话上实测：顶部挂载第 1–6 轮，底部挂载第 4–9 轮；从顶部开始按“把最后一个已处理轮次滚入视口”的方式推进，4 次加载按顺序拿到全部 9 轮，每轮 user 和 assistant 复制按钮都存在。

## 总体流程

```text
手动触发
  → 绑定当前标签页
  → Background JavaScript 初始化变量
  → Scroll element 把 .thread-scroll-container 滚到顶部 → 等待 1000ms
  → Loop elements 遍历当前挂载的 [data-turn-key] 并逐段向下滚动
      → Attribute value 读取 data-turn-key
      → Background JavaScript 检查是否已导出
          ├─ 已导出：直接进入 Loop breakpoint
          └─ 未导出：滚到视口中央 → 等待 300ms → 悬停 → 等待 200ms
                → 有 user 复制按钮？
                    ├─ 有：记录 user → 写入占位文本 → 点击 → 等待 500ms → 读取剪贴板 → 追加
                    │        → 回到 assistant 检查
                    └─ 无：直接检查 assistant
                → 有 assistant 复制按钮？
                    ├─ 有：记录 assistant → 写入占位文本 → 点击 → 等待 500ms → 读取剪贴板 → 追加
                    └─ 无：确认本轮至少导出一条消息，否则中止
      → Loop breakpoint 继续当前批次，或触发下一段滚动和加载
  → 导出 conversation 变量
```

复制路径共用 `igknuos → readclip → b0vvwkb`。追加后的 `afteruser` 条件根据 `currentMessageRole` 分流：刚处理完 user 时回到 assistant 检查，处理完 assistant 时进入 breakpoint。

`Loop elements` 会给已经发现的 DOM 节点添加 Automa 内部 `automa-loop` 标记。当前批次处理完后，它把最后一个已标记节点滚入视口，等待 `500ms`，再在 `actionElMaxWaitTime`（`5s`）内等待新挂载且未标记的轮次；等不到时循环结束，经 breakpoint 进入导出。

虚拟列表卸载后重新挂载的旧轮次没有 `automa-loop` 标记，会被当作新节点再次进入循环。后台用 `seenTurnIds` 记录已处理的 `data-turn-key`，命中时直接跳到 breakpoint，不滚动、不复制。

## 关键节点

| 节点 ID | Automa 节点 | 作用 |
| --- | --- | --- |
| `jiglnit` | JavaScript / Background | 初始化 `conversation`、`seenTurnIds` 和当前轮次变量 |
| `scrolltop` | Scroll element | 把 `.thread-scroll-container` 设为 `scrollY: -100000000`，滚到对话顶部 |
| `topwait` | Delay | 等待顶部轮次挂载 |
| `loopturn` | Loop elements | 遍历 `[data-turn-key]`，以 `scroll` 方式逐段加载 |
| `turnid` | Attribute value | 把当前轮次的 `data-turn-key` 写入 `currentTurnId` |
| `4xsplhe` | JavaScript / Background | 判断是否已处理，记录 ID，重置本轮计数 |
| `yzpfobl` | Conditions：Seen | 已处理的轮次直接进入 breakpoint |
| `turnscroll` | Scroll element | 把当前轮次滚到视口中央，确保内容已挂载 |
| `itnk3lc` | Hover element | 让当前轮次的操作按钮显示出来 |
| `hasuser` | Conditions：Has User | 判断是否有 user 复制按钮 |
| `setuser`、`clruser`、`clickuser` | JavaScript / Clipboard / Click element | 记录角色、写入占位文本、点击 user 复制按钮 |
| `hasasst` | Conditions：Has Assistant | 判断是否有 assistant 复制按钮 |
| `setasst`、`clrasst`、`clickasst` | JavaScript / Clipboard / Click element | 记录角色、写入占位文本、点击 assistant 复制按钮 |
| `igknuos` | Delay | 等待复制动作写入系统剪贴板 |
| `readclip` | Clipboard / Get | 将剪贴板文本写入 `currentCopiedText` |
| `b0vvwkb` | JavaScript / Background | 校验内容并追加消息 |
| `afteruser` | Conditions：After User | user 处理完后继续 assistant，否则进入 breakpoint |
| `turncheck` | JavaScript / Background | 一轮中两种按钮都没有时中止工作流 |
| `cqwhcdl` | Loop breakpoint | 继续当前批次，或触发下一段滚动和加载 |
| `ddyc51w` | Export data | 将 `conversation` 变量导出为 JSON |

## 页面选择器

| 用途 | 当前选择器 | 调整时注意 |
| --- | --- | --- |
| 滚动容器 | `.thread-scroll-container` | 仅用于开始前滚到顶部；若改名，`scrolltop` 会等待 5s 后报 `element-not-found` |
| 轮次容器 | `[data-turn-key]` | 必须是单个复合选择器；Automa 会在整个字符串末尾追加 `:not([automa-loop*="…"])`，若改成逗号列表需要包一层 `:is(...)` |
| user 复制按钮 | `{{loopData@ChTurn}} button:is([aria-label="复制消息"], [aria-label="Copy message"])` | `hasuser` 和 `clickuser` 必须使用同一个选择器 |
| assistant 复制按钮 | `{{loopData@ChTurn}} .turn-action-controls button:is([aria-label="复制"], [aria-label="Copy"])` | 去掉 `.turn-action-controls` 会误点代码块的复制按钮；`hasasst` 和 `clickasst` 必须一致 |

`{{loopData@ChTurn}}` 在运行时展开为 `[automa-loop="…"]`，因此按钮选择器都限定在当前轮次内。中文标签已在页面上核实；英文标签 `Copy message` / `Copy` 是按界面命名推测的，英文界面首次使用时需要确认。

## 状态与输出

Automa 变量：

- `conversation`：最终消息数组；
- `seenTurnIds`：已经处理的 `data-turn-key`；
- `currentTurnId`：当前轮次的 `data-turn-key`；
- `currentTurnSkip`：当前轮次是否已处理，取值 `yes` / `no`；
- `currentMessageRole`：正在复制的消息角色，也用于 `afteruser` 分流；
- `currentCopiedText`：从剪贴板读取的本轮复制内容；
- `turnMessageCount`：当前轮次已经导出的消息数。

每条消息输出为：

```json
{
  "role": "assistant",
  "content": "消息 Markdown",
  "turnId": "来源轮次的 data-turn-key"
}
```

同一轮的 user 和 assistant 共享 `turnId`。消息按页面 DOM 顺序追加，新页面不再提供轮次序号。Palace 当前只消费 `role` 和 `content`，`turnId` 用于排障和核对来源；修改工作流时不要改变这两个核心字段的语义。旧版的 DOM fallback 和 `copied: false` 已移除：找不到复制按钮时应中止并修正选择器，而不是导出纯文本。

## 时序与虚拟列表边界

- 开始前滚到顶部并等待 `1000ms`，让顶部轮次完成挂载。
- 每轮先把轮次滚到视口中央并等待 `300ms`，再悬停并等待 `200ms`。
- 点击复制后等待 `500ms`，随后读取系统剪贴板。
- `Loop elements` 的加载动作为 `scroll`，`scrollToBottom` 为 `false`，不会一次跳到页面底部。
- 每次滚动后先等 `500ms`，再最多等 `5s` 寻找新轮次；ChatGPT 挂载更慢时循环可能提前结束，可调大 `actionElMaxWaitTime`。
- `maxLoop` 为 `2000`，页面结构异常时也不会无限执行。
- 一轮中若有多个 assistant 操作栏，例如重新生成后的多个版本，只点击 DOM 中第一个匹配的按钮。

## 失败处理与排障

- 看到上文的 CSP 报错，说明导入的仍是含 Active Tab JavaScript 的旧工作流；开启 Debug mode 不能绕过 CSP。
- 导出缺少开头几轮：确认 `scrolltop` 是否真的到顶；容器若不再是 `column-reverse`，负值仍会被截到顶部，但需重新核对滚动容器选择器。
- `Loop elements` 报 `element-not-found`：页面上没有 `[data-turn-key]`，说明轮次容器又改版了，按下文重新确认结构。
- 若看到 `no-clipboard-acces`，在 Automa 中授予 Clipboard 权限后重试。
- `本轮没有从剪贴板读到新的复制内容`：点击后剪贴板仍是占位文本。检查复制按钮选择器，或适当增加点击后的等待时间。
- `turn … 中没有找到 user 或 assistant 的复制按钮`：检查按钮的 `aria-label` 和 `.turn-action-controls` 是否仍存在。
- 若导出在中途结束，先观察是否确实逐段滚动；必要时增加 Loop elements 的等待时间。
- 工作流的全局错误策略是停止执行，避免生成内容与角色错位但看似成功的文件。

## 站点改版后的调整顺序

1. 在浏览器控制台从复制按钮向上逐层打印祖先的标签和 `data-*` 属性，确认轮次容器、稳定 ID、两种复制按钮和它们的相对位置。DevTools 控制台执行的代码不受页面 CSP 限制。
2. 若轮次容器或 ID 属性改名，同步修改 Loop elements 和 `turnid`。
3. 若复制按钮改变，同步修改对应的 Has 条件和 Click element，并确认不会命中代码块工具栏的按钮。
4. 用至少包含 user、assistant、代码块、长回复和足以触发虚拟滚动的对话回归。
5. 检查导出数组无重复、顺序正确、首尾完整，随后用 Palace 实际导入一次。
6. 在 Automa 中重新导出后，用 `jq empty export-conversation.automa.json` 检查 JSON 完整性，并同步更新本文中的节点、选择器和时序说明。

## 本地回归验证

安装项目 npm 依赖后，在仓库根目录运行 `node --test automa/chatgpt/export-conversation.test.mjs`。测试读取实际工作流，确认没有在页面上下文执行的 JavaScript、循环前先把反向滚动容器滚到顶部；使用按实际页面结构构造的 jsdom 验证轮次排除、新批次查找，以及复制按钮选择器不会命中代码块按钮；并在内存变量表上执行后台脚本，覆盖一轮先 user 后 assistant 的顺序与分流、占位文本拒绝、无按钮中止和已处理轮次直达 breakpoint。它不代替 ChatGPT 登录页面上的完整运行验证。
