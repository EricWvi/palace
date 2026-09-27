# Todo 草稿与看板用例

[返回 Todo](README.md)。一个事项可同时拥有一个富文本正文和一个看板；两者都存于 `d_tiptap`，由 `d_todo.draft` 和 `d_todo.kanban` 分别引用。

## T17：为事项保存说明、材料或长文

在 Todo / Today 菜单点击 Draft：已有 `draft` 则打开同一文档；没有则先 `CreateTiptap`，再 `UpdateTodo` 回写文档 ID 并打开编辑器。此后可从事项上的草稿图标快速进入。

正文支持[共享编辑器能力](../../shared/editor-and-history.md)，含富文本、附件、自动保存和版本历史。归档事项的草稿图标仍能打开编辑器。首次创建和绑定是两次请求，非原子创建。

## T18：建立事项内部的看板

菜单 Kanban：若 `kanban=0`，创建新的 `d_tiptap`，正文结构为 `columns: string[]` 与 `columnValue: Record<string, Task[]>`，然后将 ID 写回 Todo。默认列根据当前语言为“待办 / 进行中 / 已完成”或“Backlog / In Progress / Done”。以后重复打开复用此文档。

这些是用户可改名的列，不是后端枚举状态。把卡片拖入 Done 列不会调用 Todo 的 Done 或 Complete。

## T19：建立和整理看板列

| 动作   | 用户流程               | 数据变化                                       |
| ------ | ---------------------- | ---------------------------------------------- |
| 新增列 | 点击新增入口，输入名称 | 往 `columns` 加入名称，同时建立空卡片数组      |
| 改名   | 列菜单 Rename          | 同步替换 `columns` 中名称与 `columnValue` 的键 |
| 删除列 | 列菜单 Delete → 确认   | 删除该列和列内全部卡片                         |

列名在旧结构中同时是显示名称和字典键，不是独立稳定 ID。发布版没有可靠的同名列隔离模型；迁移必须保留顺序和归属，不能只按文字合并。底层组件虽有列拖动能力，这个页面没有绑定列拖动手柄及 `columns` 重排流程；用户实际能拖动的是卡片，不能把列排序算作完整已接入功能。

## T20：管理看板卡片

1. 列菜单 Add Item，输入标题，在该列头部新增卡片；默认优先级 `low`。
2. 卡片菜单可改名，切换 `low / medium / high`，填写 / 修改纯文本详情，或确认删除。
3. 有详情的标题可悬停或在移动端通过弹出层查看。

卡片有字符串 ID、标题、优先级、可选 `detail` 和 `dueDate`。虽然类型和展示代码保留 `dueDate`，该版卡片菜单没有设置日期的流程，不能据字段存在推断有独立卡片排期功能。

## T21：在列内和跨列移动卡片

通过拖动调整列内顺序，或跨列改变卡片归属。更新的是整份看板 JSON；不会新增 `d_todo`、不会改变父事项难度 / 排期 / 累计次数。

## T22：保存与多端冲突

看板有修改时每 5 秒调用 `UpdateTiptap`，携带上次 `ts` 做冲突检查；显式保存使用强制版本写入并关闭。冲突会提示用户并重新获取数据，不提供逐卡片合并。页面离开保护和保存行为需要与[共享文档](../../shared/editor-and-history.md)一起迁移。

删除父 Todo 或集合不会自动删除看板。看板虽共用文档存储，其编辑界面没有富文本编辑器同样的历史浏览按钮；不能把“数据有 history”写成“看板已有历史恢复 UI”。

## 迁移核对

- 同一 Todo 的 Draft 与 Kanban 必须分别打开正确内容，不能将看板 JSON 当正文解析。
- 列删除、卡片删除、父事项删除是三个不同范围。
- 拖动顺序、卡片优先级与详情完整保留，父事项执行状态独立。

源码：[Todo 入口](../../../../third_party/dashboard/client/src/components/todo/todo-entry.tsx)、[看板数据结构](../../../../third_party/dashboard/client/src/hooks/use-kanban.ts)、[看板交互](../../../../third_party/dashboard/client/src/components/kanban-render.tsx)、[拖动组件](../../../../third_party/dashboard/client/src/components/ui/kanban.tsx)。
