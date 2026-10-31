import { generateKeyBetween } from "fractional-indexing";
import type { components } from "./generated/api";
import { addDays, dayParam, parseDay } from "./day";

// 行事: the board the page loads, and the rules that read it against the reader's own today.
// See specs/decisions/web/todo/0-today-and-lists-with-fractional-order.md.
export type Todo = components["schemas"]["Todo"];
export type TodoList = components["schemas"]["TodoList"];
export type TodoListInput = components["schemas"]["TodoListInput"];
export type TodoBoard = components["schemas"]["TodoBoard"];
export type TodoPlanInput = components["schemas"]["TodoPlanInput"];
export type CompletedTodo = components["schemas"]["CompletedTodo"];

// Where a task sits: the inbox, or one list by id.
export type Place = { kind: "inbox" } | { kind: "list"; id: string };
export const inbox: Place = { kind: "inbox" };

export function placeOf(todo: Todo): Place {
  return todo.list_id === null ? inbox : { kind: "list", id: todo.list_id };
}

export function samePlace(a: Place, b: Place): boolean {
  return a.kind === "inbox"
    ? b.kind === "inbox"
    : b.kind === "list" && a.id === b.id;
}

// The plan as the page shows it on `today`: a day already past lapsed into no plan at all, so
// crossing midnight needs no write.
export type Schedule =
  | { kind: "open" }
  | { kind: "shelved" }
  | { kind: "day"; day: string; done: boolean };

export function scheduleOf(todo: Todo, today: string): Schedule {
  const plan = todo.plan;
  // `YYYY-MM-DD` compares by date as plain strings.
  if (plan.kind === "day" && plan.day < today) return { kind: "open" };
  return plan;
}

export function isToday(todo: Todo, today: string): boolean {
  const schedule = scheduleOf(todo, today);
  return schedule.kind === "day" && schedule.day === today;
}

export function isDoneToday(todo: Todo, today: string): boolean {
  const schedule = scheduleOf(todo, today);
  return schedule.kind === "day" && schedule.day === today && schedule.done;
}

const weekdays = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"];

// Short words for a plan: 今日, 明日, a weekday within the week, then month and day; 搁置.
export function whenLabel(todo: Todo, today: string): string {
  const schedule = scheduleOf(todo, today);
  if (schedule.kind === "open") return "";
  if (schedule.kind === "shelved") return "搁置";
  const day = parseDay(schedule.day);
  const start = parseDay(today);
  if (!day || !start) return schedule.day;
  const days = Math.round((day.getTime() - start.getTime()) / 86_400_000);
  if (days === 0) return "今日";
  if (days === 1) return "明日";
  if (days < 7) return weekdays[day.getDay()];
  return `${day.getMonth() + 1}月${day.getDate()}日`;
}

export function tomorrowOf(today: string): string {
  const day = parseDay(today);
  return day ? dayParam(addDays(day, 1)) : today;
}

// One place's open tasks in key order. Keys compare as bytes, which is how JavaScript compares
// strings of ASCII, and the server's order already follows them; ties never occur.
export function tasksIn(board: TodoBoard, place: Place): Todo[] {
  return board.todos
    .filter((todo) => samePlace(placeOf(todo), place))
    .sort((a, b) => (a.position < b.position ? -1 : 1));
}

export type Group = { list: TodoList; todos: Todo[] };

// Lists in their own order, each with the tasks that pass `keep`, empty lists left out.
function groups(board: TodoBoard, keep: (todo: Todo) => boolean): Group[] {
  return board.lists
    .map((list) => ({
      list,
      todos: tasksIn(board, { kind: "list", id: list.id }).filter(keep),
    }))
    .filter((group) => group.todos.length > 0);
}

// 今日: the tasks on today, under their lists.
export function todayGroups(board: TodoBoard, today: string): Group[] {
  return groups(board, (todo) => isToday(todo, today));
}

// What planning offers: every open task in a list that is not set aside, in the list's order.
// Tasks already on a day are offered too, but not pickable; see `canPick`.
export function planGroups(board: TodoBoard, today: string): Group[] {
  return groups(board, (todo) => scheduleOf(todo, today).kind !== "shelved");
}

export function canPick(todo: Todo, today: string): boolean {
  return scheduleOf(todo, today).kind === "open";
}

// Tasks in one place and the lists themselves are both ordered by these keys.
type Ordered = { id: string; position: string };

// A key above every item: new, restored and moved tasks arrive on top.
export function topKey(items: Ordered[]): string {
  return generateKeyBetween(null, items[0]?.position ?? null);
}

// A key below every item: tasks sent to the bottom and new lists land there.
export function bottomKey(items: Ordered[]): string {
  return generateKeyBetween(items.at(-1)?.position ?? null, null);
}

// The key that puts `moved` at `index` among the other items: between its new neighbours, so
// only the moved item is written. A drop on the row at index `i` of `items` lands at `i` among the
// others whichever way it moved, as `arrayMove` would place it.
export function keyAt(items: Ordered[], moved: string, index: number): string {
  const others = items.filter((item) => item.id !== moved);
  const before = others[index - 1]?.position ?? null;
  const after = others[index]?.position ?? null;
  return generateKeyBetween(before, after);
}
