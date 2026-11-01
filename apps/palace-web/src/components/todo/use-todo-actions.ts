import { useQueryClient } from "@tanstack/react-query";
import { api, apiData, todoBoardOptions } from "@/lib/api";
import {
  bottomKey,
  isDoneToday,
  isToday,
  scheduleOf,
  tasksIn,
  topKey,
  type Place,
  type Todo,
  type TodoBoard,
  type TodoList,
  type TodoListInput,
  type TodoPlanInput,
} from "@/lib/todo";

// The parts of a task an edit replaces together; see `PUT /api/todos/{id}`.
export type TodoChanges = Partial<{
  list_id: string | null;
  title: string;
  link: string | null;
  position: string;
  plan: TodoPlanInput;
}>;

// The plan an edit will leave: done survives only on the same day, as the server keeps it.
function nextPlan(todo: Todo, plan: TodoPlanInput): Todo["plan"] {
  if (plan.kind !== "day") return plan;
  const done =
    todo.plan.kind === "day" && todo.plan.day === plan.day && todo.plan.done;
  return { kind: "day", day: plan.day, done };
}

function planInput(todo: Todo): TodoPlanInput {
  return todo.plan.kind === "day"
    ? { kind: "day", day: todo.plan.day }
    : todo.plan;
}

// Every change to 行事 goes through here. Each one writes the cached board of `today` first, so
// the page answers at once, then sends the request; the server's answer replaces the guess. A
// failure refetches the board, so the page never keeps a change the server refused, and is
// reported through `onError`.
export function useTodoActions(
  today: string,
  onError: (message: string) => void,
) {
  const client = useQueryClient();
  const boardKey = todoBoardOptions(today).queryKey;
  const board = () => client.getQueryData<TodoBoard>(boardKey);
  const write = (change: (board: TodoBoard) => TodoBoard) =>
    client.setQueryData<TodoBoard>(boardKey, (old) =>
      old ? change(old) : old,
    );
  const replace = (todo: Todo) =>
    write((b) => ({
      ...b,
      todos: b.todos.map((t) => (t.id === todo.id ? todo : t)),
      finished: b.finished.map((t) => (t.id === todo.id ? todo : t)),
    }));
  // Puts an open task on the board; a task restored from today's finished leaves them.
  const insert = (todo: Todo) =>
    write((b) => ({
      ...b,
      todos: [...b.todos.filter((t) => t.id !== todo.id), todo],
      finished: b.finished.filter((t) => t.id !== todo.id),
    }));
  const drop = (id: string) =>
    write((b) => ({
      ...b,
      todos: b.todos.filter((t) => t.id !== id),
      finished: b.finished.filter((t) => t.id !== id),
    }));
  const completedChanged = (list: string | null) => {
    if (list)
      void client.invalidateQueries({ queryKey: ["todos", "completed", list] });
  };

  // Runs a request after its optimistic write; on failure the board is fetched again.
  async function send<T>(request: () => Promise<T>): Promise<T | undefined> {
    try {
      return await request();
    } catch (error) {
      void client.invalidateQueries({ queryKey: boardKey });
      onError(
        error instanceof Error ? error.message : "保存失败，请稍后重试。",
      );
      return undefined;
    }
  }

  async function update(todo: Todo, changes: TodoChanges) {
    const body = {
      list_id: todo.list_id,
      title: todo.title,
      link: todo.link,
      position: todo.position,
      plan: planInput(todo),
      ...changes,
    };
    replace({ ...todo, ...body, plan: nextPlan(todo, body.plan) });
    const updated = await send(() =>
      apiData(
        api.PUT("/api/todos/{id}", { params: { path: { id: todo.id } }, body }),
      ),
    );
    if (updated) replace(updated);
  }

  async function restoreAt(
    id: string,
    list: string,
    position: string,
  ): Promise<Todo | undefined> {
    const restored = await send(() =>
      apiData(
        api.POST("/api/todos/{id}/restore", {
          params: { path: { id } },
          body: { position },
        }),
      ),
    );
    if (restored) insert(restored);
    completedChanged(list);
    return restored;
  }

  async function setDone(todo: Todo, day: string, done: boolean) {
    if (todo.plan.kind !== "day" || todo.plan.done === done) return;
    replace({
      ...todo,
      plan: { ...todo.plan, done },
      done_days: todo.done_days + (done ? 1 : -1),
    });
    const updated = await send(() =>
      apiData(
        api.PUT("/api/todos/{id}/done", {
          params: { path: { id: todo.id } },
          body: { day, done },
        }),
      ),
    );
    if (updated) replace(updated);
  }

  // Back where it was in its list, unless another task took its key since; then on top.
  async function restoreInPlace(todo: Todo): Promise<Todo | undefined> {
    const current = board();
    if (!current || !todo.list_id) return undefined;
    const tasks = tasksIn(current, { kind: "list", id: todo.list_id }).filter(
      (t) => t.id !== todo.id,
    );
    const free = !tasks.some((t) => t.position === todo.position);
    return restoreAt(
      todo.id,
      todo.list_id,
      free ? todo.position : topKey(tasks),
    );
  }

  return {
    // A new task goes on top of its place, unscheduled.
    async create(place: Place, title: string) {
      const current = board();
      if (!current) return;
      const created = await send(() =>
        apiData(
          api.POST("/api/todos", {
            body: {
              list_id: place.kind === "list" ? place.id : null,
              title,
              position: topKey(tasksIn(current, place)),
            },
          }),
        ),
      );
      if (created) insert(created);
    },

    update,

    // Moves a task to the top of another place; the inbox takes no plan.
    async move(todo: Todo, place: Place) {
      const current = board();
      if (!current) return;
      await update(todo, {
        list_id: place.kind === "list" ? place.id : null,
        position: topKey(
          tasksIn(current, place).filter((t) => t.id !== todo.id),
        ),
        ...(place.kind === "inbox" ? { plan: { kind: "open" } } : {}),
      });
    },

    setDone,

    // Puts the picked tasks on `day` in one request.
    async plan(day: string, ids: string[]) {
      if (ids.length === 0) return;
      const picked = new Set(ids);
      write((b) => ({
        ...b,
        todos: b.todos.map((t) =>
          picked.has(t.id)
            ? { ...t, plan: { kind: "day", day, done: false } }
            : t,
        ),
      }));
      await send(() =>
        apiData(api.POST("/api/todos/plan", { body: { day, ids } })),
      );
      void client.invalidateQueries({ queryKey: boardKey });
    },

    // A task on today is done today as it finishes and stays in 今日, struck through; any other
    // leaves the board.
    async complete(todo: Todo): Promise<boolean> {
      drop(todo.id);
      if (isToday(todo, today))
        write((b) => ({
          ...b,
          finished: [
            ...b.finished,
            {
              ...todo,
              plan: { kind: "day", day: today, done: true },
              done_days: todo.done_days + (isDoneToday(todo, today) ? 0 : 1),
            },
          ],
        }));
      const finished = await send(async () => {
        await apiData(
          api.POST("/api/todos/{id}/complete", {
            params: { path: { id: todo.id } },
            body: { day: today },
          }),
        );
        return true;
      });
      completedChanged(todo.list_id);
      return finished === true;
    },

    // Back on top of its list, with the plan it kept: on today, done, when it was finished today.
    async restore(id: string, list: string) {
      const current = board();
      if (!current) return;
      await restoreAt(
        id,
        list,
        topKey(tasksIn(current, { kind: "list", id: list })),
      );
    },

    // A task finished today, back among the open ones where it was, still done today.
    reopen: restoreInPlace,

    // Undoing a finish puts the task back as it was before: where it was in its list, with its
    // plan, and not done today unless it already was.
    async undoComplete(todo: Todo) {
      const restored = await restoreInPlace(todo);
      if (!restored) return;
      if (isToday(todo, today)) {
        if (!isDoneToday(todo, today)) await setDone(restored, today, false);
        return;
      }
      const plan = scheduleOf(todo, today);
      if (plan.kind !== "open")
        await update(restored, { plan: planInput(todo) });
    },

    // Hides a task without telling the server; `remove` sends the deletion, and the returned
    // function undoes the hiding.
    hide(id: string): () => void {
      const before = board();
      drop(id);
      return () =>
        write((b) => ({
          ...b,
          todos: [
            ...b.todos,
            ...(before?.todos.filter((t) => t.id === id) ?? []),
          ],
          finished: [
            ...b.finished,
            ...(before?.finished.filter((t) => t.id === id) ?? []),
          ],
        }));
    },
    async remove(id: string, list: string | null) {
      await send(() =>
        apiData(api.DELETE("/api/todos/{id}", { params: { path: { id } } })),
      );
      completedChanged(list);
    },

    async setBody(todo: Todo, body: string | null) {
      replace({ ...todo, has_body: body !== null });
      await send(() =>
        apiData(
          api.PUT("/api/todos/{id}/body", {
            params: { path: { id: todo.id } },
            body: { body },
          }),
        ),
      );
      client.setQueryData(["todos", "body", todo.id], { body });
    },

    async clearCompleted(list: string) {
      client.setQueryData(["todos", "completed", list], { items: [] });
      await send(() =>
        apiData(
          api.DELETE("/api/todo-lists/{id}/completed", {
            params: { path: { id: list } },
          }),
        ),
      );
      completedChanged(list);
    },

    // A new list goes after the others.
    async createList(name: string) {
      const current = board();
      if (!current) return undefined;
      const body = { name, position: bottomKey(current.lists) };
      const list = await send(() =>
        apiData(api.POST("/api/todo-lists", { body })),
      );
      if (list) write((b) => ({ ...b, lists: [...b.lists, list] }));
      return list;
    },

    // Renaming keeps the key and moving keeps the name; either writes the list whole.
    async updateList(list: TodoList, changes: Partial<TodoListInput>) {
      const body = { name: list.name, position: list.position, ...changes };
      write((b) => ({
        ...b,
        lists: b.lists
          .map((l) => (l.id === list.id ? { ...l, ...body } : l))
          .sort((x, y) => (x.position < y.position ? -1 : 1)),
      }));
      await send(() =>
        apiData(
          api.PUT("/api/todo-lists/{id}", {
            params: { path: { id: list.id } },
            body,
          }),
        ),
      );
    },

    // The list's tasks, finished or not, go with it.
    async deleteList(id: string) {
      write((b) => ({
        lists: b.lists.filter((l) => l.id !== id),
        todos: b.todos.filter((t) => t.list_id !== id),
        finished: b.finished.filter((t) => t.list_id !== id),
      }));
      await send(() =>
        apiData(
          api.DELETE("/api/todo-lists/{id}", { params: { path: { id } } }),
        ),
      );
      client.removeQueries({ queryKey: ["todos", "completed", id] });
    },
  };
}

export type TodoActions = ReturnType<typeof useTodoActions>;
