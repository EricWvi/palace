# Automa 示例工作流维护

修改某个站点的工作流前，先阅读同目录的 `export-conversation.md`；修改行为后同步更新该文档。工作流 JSON 是实际运行配置，回归测试应直接读取其中的选择器、脚本或连线。

## 排查时先取得 Automa 源码

遇到节点行为与配置含义不一致、脚本注入失败、循环不退出或条件分支异常时，clone Automa 官方源码，沿实际执行路径核实行为。仅凭节点名称、UI 文案或自行模拟的逻辑不足以判断根因。

将源码放到独立临时目录，不纳入 Palace 仓库：

```bash
automa_debug_dir=$(mktemp -d)
git clone --depth 1 https://github.com/AutomaApp/automa.git "$automa_debug_dir/automa"
git -C "$automa_debug_dir/automa" rev-parse HEAD
```

核对用户安装的扩展版本、工作流的 `extVersion` / `version` 和源码 `package.json`，并记录分析所用 commit。默认分支可能与已安装版本不同；需要时 fetch 对应 tag 或 commit 后再分析。

本次排查参考 commit 为 `a4cbe34a60c92873c48c2470ca8ab1d96c22c7a0`，其 `package.json` 标注 `1.30.00`，工作流标注 `1.30.02`。这些信息用于追溯，不代表两个版本完全一致；下面的源码路径也应随所查版本重新确认。

| 排查内容                                   | 优先查看的源码路径                                                                                       |
| ------------------------------------------ | -------------------------------------------------------------------------------------------------------- |
| 元素循环查找下一批、排除已处理节点、滚动   | `src/content/blocksHandler/handlerLoopElements.js`                                                       |
| 循环索引、批次状态、加载参数               | `src/workflowEngine/blocksHandler/handlerLoopElements.js`                                                |
| 循环继续、`clearLoop` 和加载更多的先后顺序 | `src/workflowEngine/blocksHandler/handlerLoopBreakpoint.js`                                              |
| DOM 标记及逐项选择器生成                   | `src/content/utils.js` 中的 `generateLoopSelectors`                                                      |
| Conditions 计算与输出分支                  | `src/workflowEngine/blocksHandler/handlerConditions.js`、`src/workflowEngine/utils/testConditions.js`    |
| JavaScript 执行上下文、变量回传与注入      | `src/workflowEngine/blocksHandler/handlerJavascriptCode.js`，再沿调用查找 sandbox、background 和注入工具 |

## 用实际失败场景验证

1. 明确顺序和角色。例如“首轮问答完整，第二轮从顶部开始仅复制全部 user”与“反复复制第一条”是两个不同场景。
2. 检查 Automa 如何加工输入配置，尤其是选择器拼接、DOM 标记和分支连线。保留源码中的关键行为，用实际工作流配置构造最小复现。
3. 先让测试在旧配置上失败，再应用修改并通过同一测试。选择器问题使用真实 DOM 查询；只执行消息数组去重脚本无法验证 DOM 遍历是否正确。
4. 说明验证边界：本地测试验证配置和执行规则，浏览器中的滚动、剪贴板、权限及站点改版仍需实际运行确认。

## 用控制台探测脚本定位页面问题

Agent 无法直接访问用户登录后的站点。页面结构、虚拟列表或滚动行为不明时，给用户一段只读的 DevTools 控制台脚本，由用户在目标页面执行并把结果贴回。DevTools 控制台执行的代码不受页面 CSP 或 Trusted Types 限制，因此能在 Automa 注入失败的页面上探测；反过来，控制台能跑通也不代表 Automa 注入能跑通。

### 脚本约定

- 只读：不点击、不复制、不修改页面，最多滚动。需要模拟 Automa 行为时，只复现它的查找和滚动规则。
- 结果汇总为一个 JSON 对象，`console.log` 打印的同时自动复制到剪贴板，用户直接粘贴即可，不必展开控制台对象手抄。
- 默认不收集消息正文：只收集标签、属性（值截断到约 80 字符，`class` 截到 60 字符并去掉 `style`）、数量和滚动位置，长 ID 只保留前 8 位。确需正文片段核对角色或顺序时，限制在 20 个字符以内，并提醒用户可在粘贴前删除。
- 同步脚本用 IIFE 包裹；需要等待的脚本用 `await (async () => { … })()`，并限制最大轮数和等待时间。
- 回复中说明每个字段怎么解读，以及不同结果分别导向哪一步修改，让一次执行就能决定下一步。

### 自动复制结果

DevTools 的 `copy()` 属于 Command Line API，只能在控制台里调用。在同步脚本中可以直接用；在 async 脚本里执行过 `await` 后可能报 `ReferenceError: copy is not defined`。页面自身的 `navigator.clipboard.writeText` 在焦点位于 DevTools 时也会因文档未聚焦而失败。因此异步脚本应先把报告挂到 `window` 上，再尝试复制，失败时让用户单独执行一次 `copy(...)`：

```js
await (async () => {
  const report = {};
  // … 收集并等待 …
  window.__automaProbe = report;
  console.log(report);
  try {
    copy(JSON.stringify(report, null, 2));
    return "report copied to clipboard";
  } catch {
    return "run: copy(JSON.stringify(__automaProbe, null, 2))";
  }
})();
```

### 按层次逐步探测

每一步只回答一个问题，拿到结果再写下一段脚本，不要一次猜测全部结构。以 2026 年 9 月 ChatGPT 改版为例：

1. **现有选择器还能否命中**：对工作流用到的每个选择器及其逐级放宽的版本（去掉属性、换标签、只留属性）打印 `querySelectorAll(...).length`，同时列出可能的复制按钮的 `aria-label`。哪一级开始降为 0，就是哪个条件失效。当时所有 `data-testid` / `data-turn*` / `data-message-author-role` 都是 0，只有按 `aria-label` 能找到复制按钮，说明是整体改版。
2. **从可见锚点向上找结构**：从每个复制按钮向上逐层打印约 12 层祖先的标签、`data-*` 属性、子元素数和 `innerText` 长度；求两个按钮的最近公共祖先并列出其子元素；统计链上出现过的 `data-*`、`id`、`role`；列出 `overflow-y` 为 `auto` / `scroll` 且可滚动的元素。由此确定循环容器（`[data-turn-key]`）、稳定 ID、两种按钮的相对位置，并发现代码块工具栏里同名的“复制”按钮需要排除。
3. **挂载窗口**：在页面顶部和底部各执行一次“列出当前容器及每个容器内按钮数量”的短脚本。两次结果的并集和重叠部分说明虚拟列表挂载哪些条目、窗口是否连续。
4. **模拟 Automa 的加载方式**：先滚到顶部，再照搬 `handlerLoopElements.js` 的规则循环：把 DOM 中最后一个已记录条目 `scrollIntoView()`，等 `500ms`，再在 `actionElMaxWaitTime` 内轮询新条目，没有新条目时结束。报告总数、是否到底、每次新增的条目和当时的 `scrollTop`，并且只在有新增时记录一轮，避免轮询把报告撑大。
5. **滚动方向**：第 4 步第一次只拿到底部几条，`scrollTop` 为负数。在报告中加入容器及其子元素的 `getComputedStyle(...).flexDirection` 后确认是 `column-reverse`，修正“回到顶部”的写法后即完整拿到全部 9 轮。若探测结果与预期不符，先怀疑探测脚本本身的假设，再怀疑工作流。

探测结论要落到工作流、站点维护文档和回归测试：用观察到的 DOM 结构构造 jsdom 夹具，包括需要排除的干扰元素。

## 已确认的陷阱

- **逗号选择器与追加条件**：所查 Automa 的 `excludeSelector` 会直接在选择器末尾追加 `:not([automa-loop*="…"])`。`user-query, model-response` 因而只排除已处理的 assistant，全部 user 仍被重新选中；处理首条 user 又把页面带回顶部。使用 `:is(user-query, model-response)`，让追加条件同时作用于两类消息。这个问题在静态 DOM 上即可复现，应先检查它，再调查虚拟列表卸载导致标记丢失的可能性。
- **`clearLoop` 不一定立即退出**：所查 handler 在 `clearLoop: true` 时仍可能执行元素循环的 `loadMoreAction`，发现节点后再次进入循环。Gemini 的回卷确认分支因此直接连接 Export。检查退出行为时，要验证实际下一节点，而不只是变量或标志是否正确。
- **Trusted Types 与脚本注入**：`HTMLScriptElement.textContent` 的 `TrustedScript` 报错发生在注入阶段；控制台直接执行脚本成功，不代表 Automa 的注入方式也能成功。开启 Debug mode 也不能据此认定问题已解决。Gemini 当前使用原生页面节点操作 DOM，Background JavaScript 整理变量，具体限制见站点维护文档。
- **CSP 与脚本注入**：ChatGPT 的 `script-src-elem` 只允许 nonce 或 hash，Active Tab JavaScript 注入的内联脚本会被拦截。处理方式与 Trusted Types 相同：页面操作改用原生节点，JavaScript 只在 Background 上下文运行。新工作流默认不要使用 Active Tab JavaScript。
- **报 `element-not-found` 先查页面结构**：循环节点在第一批就找不到元素时，通常是站点改版而不是时序问题。按上文“按层次逐步探测”的第 1、2 步重新确定容器、稳定 ID 和按钮位置。
- **反向滚动容器**：聊天页常用 `flex-direction: column-reverse`，此时 `scrollTop` 为 0 表示底部、向上为负。探测脚本或 Scroll element 要回到顶部时应设置足够大的负值，先用 `getComputedStyle` 确认方向，否则会误判“滚动加载推不动”。
- **文本重复不等于同一消息**：合法的多次“继续”应保留。内容回卷检测只能兜底；已确认是选择器或节点控制流的问题时，应修正该根因，而不是不断增加内容匹配阈值。

## 保留回归测试

`gemini/export-conversation.test.mjs` 使用项目依赖中的 jsdom，读取实际工作流，覆盖首轮顺序、已处理节点排除、新批次查找和回卷分支直达导出。`chatgpt/export-conversation.test.mjs` 覆盖无页面上下文脚本、复制按钮选择器不误中代码块按钮，以及后台脚本的去重、user/assistant 分流与失败判定。保留这些文件并纳入 Git；修改对应工作流的选择器、脚本或连线后，在仓库根目录运行：

```bash
node --test automa/*/export-conversation.test.mjs
jq empty automa/*/export-conversation.automa.json
git diff --check
```

测试通过后仍需区分“本地回归通过”和“用户浏览器实测通过”。本次选择器修复已获得用户实测确认。
