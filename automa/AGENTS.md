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

## 已确认的陷阱

- **逗号选择器与追加条件**：所查 Automa 的 `excludeSelector` 会直接在选择器末尾追加 `:not([automa-loop*="…"])`。`user-query, model-response` 因而只排除已处理的 assistant，全部 user 仍被重新选中；处理首条 user 又把页面带回顶部。使用 `:is(user-query, model-response)`，让追加条件同时作用于两类消息。这个问题在静态 DOM 上即可复现，应先检查它，再调查虚拟列表卸载导致标记丢失的可能性。
- **`clearLoop` 不一定立即退出**：所查 handler 在 `clearLoop: true` 时仍可能执行元素循环的 `loadMoreAction`，发现节点后再次进入循环。Gemini 的回卷确认分支因此直接连接 Export。检查退出行为时，要验证实际下一节点，而不只是变量或标志是否正确。
- **Trusted Types 与脚本注入**：`HTMLScriptElement.textContent` 的 `TrustedScript` 报错发生在注入阶段；控制台直接执行脚本成功，不代表 Automa 的注入方式也能成功。开启 Debug mode 也不能据此认定问题已解决。Gemini 当前使用原生页面节点操作 DOM，Background JavaScript 整理变量，具体限制见站点维护文档。
- **文本重复不等于同一消息**：合法的多次“继续”应保留。内容回卷检测只能兜底；已确认是选择器或节点控制流的问题时，应修正该根因，而不是不断增加内容匹配阈值。

## 保留回归测试

`gemini/export-conversation.test.mjs` 使用项目依赖中的 jsdom，读取实际工作流，覆盖首轮顺序、已处理节点排除、新批次查找和回卷分支直达导出。保留该文件并纳入 Git；修改 Gemini 的选择器或退出连线后，在仓库根目录运行：

```bash
node --test examples/gemini/export-conversation.test.mjs
jq empty examples/gemini/export-conversation.automa.json
git diff --check
```

测试通过后仍需区分“本地回归通过”和“用户浏览器实测通过”。本次选择器修复已获得用户实测确认。
