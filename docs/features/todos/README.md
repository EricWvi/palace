# 行事

任务（Todo）的管理页：今日安排、收件箱和各个集合。决策见[行事 ADR](../../../specs/decisions/web/todo/0-today-and-lists-with-fractional-order.md)，交互原型见 [`design/5.1-todo.html`](../../../design/5.1-todo.html)。旧 Dashboard 的行为整理见 [Todo 旧版文档](../../legacy/dashboard/todo/README.md)；难易程度、看板、Journey 自动添加和 TimeTagger 计时都没有移植，旧数据也没有迁移。

## 一条任务

- **位置**：收件箱，或一个集合。收件箱是还没想好放哪的缓冲区，里面的任务不能排期、不能完成。
- **排期**：未排期、搁置、某一天三选一。排到的日子早于今天时按未排期处理，可以再安排。搁置的任务留在集合里，标题淡一档，安排今日时不列出。
- **今日已做**：只对排在今天的任务。勾上时做过的天数加一，取消时减一；改排期会清掉它，天数不变。
- **完成**：任务进入集合底部的「已完成」，排期清除，天数保留；恢复后回到集合顶部，未排期。
- **链接与正文**：链接让标题可点击；正文是 Markdown 纯文本，在侧栏里编辑。
- **顺序**：每个位置内按 `position` 的字节序排列。`position` 是 [`fractional-indexing`](https://github.com/rocicorp/fractional-indexing) 的键，只由页面生成；同一位置的未完成任务键不重复。

「今天」是浏览器的本地日期。服务端只保存日期，不判断哪天是今天；今日已做的请求带上页面的日期，与排期不符时返回 409。

## 接口

| 方法与路径 | 作用 |
| --- | --- |
| `GET /api/todos` | `{ lists, todos }`：全部集合（按创建顺序）与全部未完成任务 |
| `POST /api/todos` | `{ list_id, title, position }`，`list_id` 为 `null` 是收件箱；返回 201 |
| `PUT /api/todos/{id}` | `{ list_id, title, link, position, plan }` 一起替换 |
| `DELETE /api/todos/{id}` | 删除，返回 204 |
| `GET`/`PUT /api/todos/{id}/body` | `{ body }`，`null` 表示没有或清除 |
| `PUT /api/todos/{id}/done` | `{ day, done }` |
| `POST /api/todos/{id}/complete` | 完成，返回 204 |
| `POST /api/todos/{id}/restore` | `{ position }`，恢复 |
| `POST /api/todos/plan` | `{ day, ids }`，把其中未排期的任务排到这一天，最多 1000 个 |
| `POST /api/todo-lists`、`PUT /api/todo-lists/{id}` | `{ name }`，新建、改名 |
| `DELETE /api/todo-lists/{id}` | 连同其中全部任务删除 |
| `GET`/`DELETE /api/todo-lists/{id}/completed` | 已完成列表（最近完成的在前）、清空 |

`plan` 带 `kind`：`{ "kind": "open" }`、`{ "kind": "shelved" }`、`{ "kind": "day", "day": "2026-10-23", "done": false }`；写入时 `day` 不带 `done`。其他 Owner 的任务和集合都返回 404。字段的完整定义见 [OpenAPI](../../../contracts/openapi.json)。

## 页面

- 地址：`/todos` 今日，`/todos/inbox` 收件箱，`/todos/lists/{id}` 集合。页面宽 920px。
- 左列：今日（已做 / 总数）、收件箱、集合（未完成数），末尾「＋」原地新建集合；右键集合可以改名或删除。
- 今日：按集合分组列出排在今天的任务，点圆圈切换今日已做。「安排」或 `p` 原地列出可安排的任务，勾选后「排入今日」。
- 集合：拖动排序（也可以用键盘：`Space` 拿起，方向键移动，`Space` 放下），越过首行或末行时落在那一端。`n` 在顶部打开新建输入框，回车添加后可以接着输入下一条，输入框为空时回车或 `Esc` 关闭；在今日按 `n` 时转到收件箱新建。
- 任务菜单（右键或悬停出现的 `···`）：今日已做、完成、排期（今日、明日、选日期、搁置或取消搁置、清除排期）、改名、链接、正文、置顶、置底、移到、删除。
- 完成和删除后底部出现提示和「撤销」。删除在提示消失（约 4 秒）或被下一条提示取代时才发出请求。
- 所有修改先显示在页面上再发请求；失败时重新读取，并在底部提示原因。

## 工程入口

- 数据库：`crates/db/migrations/0020_todos.sql`、`crates/db/src/todo.rs`、`crates/db/src/todo_list.rs`；校验：`crates/domain/src/todo.rs`、`crates/domain/src/todo/position.rs`。
- 接口：`crates/backend/src/http/todos.rs`、`crates/backend/src/http/todo_lists.rs`。
- 前端：`apps/palace-web/src/pages/todo.tsx`、`lib/todo.ts`、`components/todo/`、`todo.css`。
