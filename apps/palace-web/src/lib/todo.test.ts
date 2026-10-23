import { expect, it } from "vitest";
import {
  bottomKey,
  canPick,
  keyAt,
  planGroups,
  scheduleOf,
  tasksIn,
  todayGroups,
  topKey,
  whenLabel,
  type Todo,
  type TodoBoard,
} from "./todo";

const today = "2026-10-23";
const task = (
  id: string,
  position: string,
  plan: Todo["plan"] = { kind: "open" },
  list_id: string | null = "work",
): Todo => ({
  id,
  list_id,
  title: id,
  position,
  link: null,
  has_body: false,
  plan,
  done_days: 0,
});
const on = (day: string, done = false): Todo["plan"] => ({
  kind: "day",
  day,
  done,
});

// specs/test-cases/web/todo/todos.md#a-task-must-keep-one-plan-and-the-inbox-none
it("reads a lapsed day as no plan and names the rest in short words", () => {
  const plans: Todo["plan"][] = [
    on("2026-10-22", true),
    on(today),
    on("2026-10-24"),
    on("2026-10-29"),
    on("2026-10-30"),
    { kind: "shelved" },
    { kind: "open" },
  ];
  const tasks = plans.map((plan, i) => task(`t${i}`, `a${i}`, plan));
  expect(tasks.map((todo) => scheduleOf(todo, today).kind)).toEqual([
    "open",
    "day",
    "day",
    "day",
    "day",
    "shelved",
    "open",
  ]);
  expect(tasks.map((todo) => whenLabel(todo, today))).toEqual([
    "",
    "今日",
    "明日",
    "周四",
    "10月30日",
    "搁置",
    "",
  ]);
  expect(tasks.map((todo) => canPick(todo, today))).toEqual([
    true,
    false,
    false,
    false,
    false,
    false,
    true,
  ]);
});

it("groups today and planning by list order, each list in key order", () => {
  const board: TodoBoard = {
    lists: [
      { id: "work", name: "工作" },
      { id: "life", name: "个人" },
      { id: "empty", name: "空" },
    ],
    todos: [
      task("late", "a1", on(today), "life"),
      task("early", "Zz", on(today), "life"),
      task("shelved", "a0", { kind: "shelved" }, "work"),
      task("next", "a2", on("2026-10-24"), "work"),
      task("inbox", "a0", { kind: "open" }, null),
    ],
  };
  const names = (groups: ReturnType<typeof todayGroups>) =>
    groups.map((g) => [g.list.name, g.todos.map((t) => t.id)]);
  expect(names(todayGroups(board, today))).toEqual([
    ["个人", ["early", "late"]],
  ]);
  expect(names(planGroups(board, today))).toEqual([
    ["工作", ["next"]],
    ["个人", ["early", "late"]],
  ]);
  expect(tasksIn(board, { kind: "inbox" }).map((t) => t.id)).toEqual(["inbox"]);
});

// specs/test-cases/web/todo/todos.md#task-order-must-come-from-unique-well-formed-keys
it("writes one key between the new neighbours, at either end too", () => {
  const tasks = ["a0", "a1", "a2", "a3"].map((key) => task(key, key));
  const sorted = (moved: string, key: string) =>
    tasks
      .map((todo) => (todo.id === moved ? { ...todo, position: key } : todo))
      .sort((a, b) => (a.position < b.position ? -1 : 1))
      .map((todo) => todo.id);
  // Down onto the third row, up onto the second, onto the first and onto the last.
  expect(sorted("a0", keyAt(tasks, "a0", 2))).toEqual(["a1", "a2", "a0", "a3"]);
  expect(sorted("a3", keyAt(tasks, "a3", 1))).toEqual(["a0", "a3", "a1", "a2"]);
  expect(sorted("a2", keyAt(tasks, "a2", 0))).toEqual(["a2", "a0", "a1", "a3"]);
  expect(sorted("a0", keyAt(tasks, "a0", 3))).toEqual(["a1", "a2", "a3", "a0"]);
  expect([topKey(tasks), bottomKey(tasks), topKey([])]).toEqual([
    "Zz",
    "a4",
    "a0",
  ]);
});
