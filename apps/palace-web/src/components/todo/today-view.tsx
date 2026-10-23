import { parseDay } from "@/lib/day";
import {
  canPick,
  isDoneToday,
  isToday,
  planGroups,
  todayGroups,
  whenLabel,
  type Group,
} from "@/lib/todo";
import { useTodoPage } from "./todo-context";
import { TodoRow } from "./todo-row";

const weekdays = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"];

function GroupName({ group }: { group: Group }) {
  return <h2 className="todo-group">{group.list.name}</h2>;
}

/*
 * Planning, drawn where today's tasks were: every candidate under its list, in the list's order.
 * Tasks already on a day are shown with it but cannot be picked, so the whole picture stays in
 * view; their own menu moves them.
 */
function PlanBody({
  picked,
  onPick,
}: {
  picked: Set<string>;
  onPick: (id: string) => void;
}) {
  const { board, today } = useTodoPage();
  const groups = planGroups(board, today);
  if (groups.length === 0)
    return <p className="todo-empty">集合里没有可以安排的任务。</p>;
  return groups.map((group) => (
    <section key={group.list.id}>
      <GroupName group={group} />
      <ul className="todo-rows">
        {group.todos.map((todo) => {
          const pickable = canPick(todo, today);
          return (
            <li
              key={todo.id}
              className={
                pickable ? "todo-row pick-row" : "todo-row pick-row taken"
              }
            >
              <label>
                <input
                  type="checkbox"
                  className="pick"
                  disabled={!pickable}
                  checked={!pickable || picked.has(todo.id)}
                  onChange={() => onPick(todo.id)}
                />
                <span className="todo-title">{todo.title}</span>
              </label>
              {!pickable && (
                <span className="todo-aside">
                  <span
                    className={isToday(todo, today) ? "when today" : "when"}
                  >
                    {whenLabel(todo, today)}
                  </span>
                </span>
              )}
            </li>
          );
        })}
      </ul>
    </section>
  ));
}

// 今日: the date and how much of today is done, then today's tasks under their lists. While
// planning, the same place lists what can be put on today.
export function TodayView({
  planning,
  picked,
  onPick,
  onPlan,
  onCancel,
  onStart,
}: {
  planning: boolean;
  picked: Set<string>;
  onPick: (id: string) => void;
  onPlan: () => void;
  onCancel: () => void;
  onStart: () => void;
}) {
  const { board, today } = useTodoPage();
  const groups = todayGroups(board, today);
  const tasks = groups.flatMap((group) => group.todos);
  const done = tasks.filter((todo) => isDoneToday(todo, today)).length;
  const day = parseDay(today);
  const facts = day
    ? [`${day.getMonth() + 1}月${day.getDate()}日`, weekdays[day.getDay()]]
    : [];
  if (tasks.length > 0) facts.push(`${done} / ${tasks.length}`);
  return (
    <>
      <div className="todo-head">
        <h1>今日</h1>
        <p className="meta">
          {facts.map((fact) => (
            <span key={fact}>{fact}</span>
          ))}
        </p>
        <span className="todo-head-actions">
          {planning ? (
            <>
              <button type="button" onClick={onCancel}>
                取消
              </button>
              <button type="button" className="go" onClick={onPlan}>
                排入今日{picked.size > 0 ? `（${picked.size}）` : ""}
              </button>
            </>
          ) : (
            <button type="button" onClick={onStart}>
              安排
            </button>
          )}
        </span>
      </div>
      {planning ? (
        <PlanBody picked={picked} onPick={onPick} />
      ) : groups.length === 0 ? (
        <p className="todo-empty">今天还没有安排。</p>
      ) : (
        groups.map((group) => (
          <section key={group.list.id}>
            <GroupName group={group} />
            <ul className="todo-rows">
              {group.todos.map((todo) => (
                <TodoRow key={todo.id} todo={todo} context="today" />
              ))}
            </ul>
          </section>
        ))
      )}
    </>
  );
}
