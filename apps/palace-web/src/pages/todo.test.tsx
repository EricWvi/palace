import { screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { address, mountApp } from "@/test-app";
import { dayParam } from "@/lib/day";
import type { Todo, TodoBoard, TodoList } from "@/lib/todo";
import { requestPath, testRequest } from "@/lib/test-request";

const today = dayParam(new Date());
const tomorrow = dayParam(
  new Date(
    new Date().getFullYear(),
    new Date().getMonth(),
    new Date().getDate() + 1,
  ),
);
const task = (
  id: string,
  list_id: string | null,
  position: string,
  plan: Todo["plan"] = { kind: "open" },
): Todo => ({
  id,
  list_id,
  title: id,
  position,
  link: null,
  has_body: false,
  plan,
  done_days: 2,
});

const lists: TodoList[] = [
  { id: "work", name: "工作", position: "a0" },
  { id: "life", name: "个人", position: "a1" },
];

// Serves the board and records every write, answering each as the server would. Tasks of a list
// left out of `shown` are left out with it.
function serve(shown: TodoList[] = lists) {
  const board: TodoBoard = {
    lists: shown,
    todos: [
      task("读书", "life", "a0", { kind: "day", day: today, done: false }),
      task("跑步", "life", "a1", { kind: "day", day: today, done: true }),
      task("搁着", "life", "a2", { kind: "shelved" }),
      task("明天的", "work", "a0", { kind: "day", day: tomorrow, done: false }),
      task("没排的", "work", "a1"),
      task("先收下", null, "a0"),
    ].filter(
      (todo) =>
        todo.list_id === null || shown.some((list) => list.id === todo.list_id),
    ),
  };
  const writes: { method: string; path: string; body: unknown }[] = [];
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
    const request = testRequest(input);
    const path = requestPath(input);
    if (request.method === "GET" && path === "/api/todos")
      return Response.json(board);
    if (request.method === "GET" && path.endsWith("/completed"))
      return Response.json({ items: [] });
    const text = await request.text();
    const body = text ? JSON.parse(text) : null;
    writes.push({
      method: request.method,
      path: decodeURIComponent(path),
      body,
    });
    if (path === "/api/todos/plan")
      for (const todo of board.todos)
        if (body.ids.includes(todo.id))
          todo.plan = { kind: "day", day: body.day, done: false };
    const id = decodeURIComponent(path.split("/")[3] ?? "");
    const todo = board.todos.find((t) => t.id === id);
    if (path.endsWith("/done") && todo && todo.plan.kind === "day")
      return Response.json({
        ...todo,
        plan: { ...todo.plan, done: body.done },
        done_days: todo.done_days + (body.done ? 1 : -1),
      });
    if (request.method === "PUT" && todo)
      return Response.json({ ...todo, ...body });
    if (request.method === "POST" && path === "/api/todos")
      return Response.json(
        { ...task(body.title, body.list_id, body.position) },
        { status: 201 },
      );
    if (request.method === "POST" && path === "/api/todo-lists")
      return Response.json({ id: "study", ...body }, { status: 201 });
    if (request.method === "PUT" && path.startsWith("/api/todo-lists/"))
      return Response.json({ id, ...body });
    return new Response(null, { status: 204 });
  });
  return writes;
}

beforeEach(() => vi.restoreAllMocks());
afterEach(() => vi.restoreAllMocks());

const row = (title: string) => screen.getByText(title).closest("li")!;

// Opens a row's `···` menu from the keyboard, as Radix menus open reliably in jsdom.
async function menuOf(
  title: string,
  user: ReturnType<typeof mountApp>["user"],
) {
  within(row(title))
    .getByRole("button", { name: `${title} 的操作` })
    .focus();
  await user.keyboard("{Enter}");
  return screen.findByRole("menu");
}

// specs/test-cases/web/todo/todos.md#todo-navigation-must-open-the-roomy-todo-page
// specs/test-cases/web/todo/todos.md#done-for-the-day-must-count-each-day-once
it("opens 行事 on today, wider, and ticks a task done for today", async () => {
  const writes = serve();
  const { user } = mountApp("/todos");
  await screen.findByText("读书");
  const nav = screen.getByRole("navigation", { name: "主导航" });
  expect(within(nav).getByRole("link", { name: "行事" })).toHaveAttribute(
    "aria-current",
    "page",
  );
  expect(screen.getByRole("main")).toHaveClass("sheet", "roomy");
  // Only today's tasks, under their list; the shelved and tomorrow's stay in their lists.
  expect(screen.getByRole("heading", { name: "个人" })).toBeInTheDocument();
  expect(screen.queryByText("明天的")).toBeNull();
  expect(screen.queryByText("搁着")).toBeNull();
  expect(screen.getByRole("link", { name: /今日/ })).toHaveTextContent("1/2");

  await user.click(screen.getByRole("checkbox", { name: "今日已做：读书" }));
  expect(
    await screen.findByRole("checkbox", {
      name: "今日已做：读书",
      checked: true,
    }),
  ).toBeInTheDocument();
  expect(writes).toEqual([
    {
      method: "PUT",
      path: "/api/todos/读书/done",
      body: { day: today, done: true },
    },
  ]);
  expect(screen.getByRole("link", { name: /今日/ })).toHaveTextContent("2/2");
});

// specs/test-cases/web/todo/todos.md#a-task-must-keep-one-plan-and-the-inbox-none
it("plans the day in place from every list but the inbox", async () => {
  const writes = serve();
  const { user } = mountApp("/todos");
  await screen.findByText("读书");
  await user.keyboard("p");
  const pickable = await screen.findByRole("checkbox", { name: "没排的" });
  // Planned tasks show their day but cannot be picked; shelved and inbox tasks are not offered.
  expect(screen.getByRole("checkbox", { name: "明天的" })).toBeDisabled();
  expect(screen.getByRole("checkbox", { name: "读书" })).toBeDisabled();
  expect(screen.queryByText("搁着")).toBeNull();
  expect(screen.queryByText("先收下")).toBeNull();
  await user.click(pickable);
  await user.click(screen.getByRole("button", { name: "排入今日（1）" }));
  expect(
    await screen.findByRole("checkbox", { name: "今日已做：没排的" }),
  ).toBeInTheDocument();
  expect(writes).toEqual([
    {
      method: "POST",
      path: "/api/todos/plan",
      body: { day: today, ids: ["没排的"] },
    },
  ]);
});

it("keeps the inbox a buffer: no plan, no finishing, and `n` from today lands there", async () => {
  serve();
  const { router, user } = mountApp("/todos");
  await screen.findByText("读书");
  await user.keyboard("n");
  await waitFor(() => expect(address(router)).toBe("/todos/inbox"));
  expect(await screen.findByRole("textbox", { name: "新任务" })).toHaveFocus();
  const menu = await menuOf("先收下", user);
  const labels = within(menu)
    .getAllByRole("menuitem")
    .map((item) => item.textContent);
  expect(labels).toEqual([
    "改名",
    "加链接…",
    "写正文",
    "置顶",
    "置底",
    "移到",
    "删除",
  ]);
  // With no plan entries, the menu opens on 改名 rather than on a lone rule.
  expect(menu.firstElementChild).toHaveAttribute("role", "menuitem");
});

it("leaves 移到 out of an inbox task's menu while there is no list to move it to", async () => {
  serve([]);
  const { user } = mountApp("/todos/inbox");
  await screen.findByText("先收下");
  const menu = await menuOf("先收下", user);
  const labels = within(menu)
    .getAllByRole("menuitem")
    .map((item) => item.textContent);
  expect(labels).toEqual(["改名", "加链接…", "写正文", "置顶", "置底", "删除"]);
});

// specs/test-cases/web/todo/todos.md#list-order-must-come-from-unique-well-formed-keys
it("adds a list after the others and renames it in place, keeping its key", async () => {
  const writes = serve();
  const { router, user } = mountApp("/todos");
  await screen.findByText("读书");
  await user.click(screen.getByRole("button", { name: "新建集合" }));
  await user.type(
    screen.getByRole("textbox", { name: "新集合" }),
    "学习{Enter}",
  );
  await waitFor(() => expect(address(router)).toBe("/todos/lists/study"));
  const side = screen.getByRole("navigation", { name: "集合" });
  await user.pointer({
    keys: "[MouseRight]",
    target: within(side).getByRole("link", { name: /^学习/ }),
  });
  await user.click(await screen.findByRole("menuitem", { name: "改名" }));
  await user.type(
    screen.getByRole("textbox", { name: "集合名" }),
    "{Control>}a{/Control}读书会{Enter}",
  );
  await waitFor(() => expect(writes).toHaveLength(2));
  const created = writes[0].body as { position: string };
  expect(created.position > "a1").toBe(true);
  expect(writes).toEqual([
    {
      method: "POST",
      path: "/api/todo-lists",
      body: { name: "学习", position: created.position },
    },
    {
      method: "PUT",
      path: "/api/todo-lists/study",
      body: { name: "读书会", position: created.position },
    },
  ]);
  expect(
    within(side)
      .getAllByRole("link")
      .map((link) => link.firstChild?.textContent),
  ).toEqual(["今日", "收件箱", "工作", "个人", "读书会"]);
});

it("adds tasks one after another and closes the field on Enter once it is empty", async () => {
  const writes = serve();
  const { user } = mountApp("/todos/lists/work");
  await screen.findByText("没排的");
  await user.keyboard("n");
  const field = await screen.findByRole("textbox", { name: "新任务" });
  await user.type(field, "第一条{Enter}");
  expect(await screen.findByText("第一条")).toBeInTheDocument();
  expect(field).toHaveValue("");
  await user.keyboard("{Enter}");
  await waitFor(() =>
    expect(screen.queryByRole("textbox", { name: "新任务" })).toBeNull(),
  );
  expect(writes.map(({ method, path }) => ({ method, path }))).toEqual([
    { method: "POST", path: "/api/todos" },
  ]);
});

// specs/test-cases/web/todo/todos.md#a-task-must-keep-one-plan-and-the-inbox-none
it("shelves a task from its menu and shows it faded with its word", async () => {
  const writes = serve();
  const { user } = mountApp("/todos/lists/work");
  await screen.findByText("没排的");
  const menu = await menuOf("没排的", user);
  within(menu).getByRole("menuitem", { name: "排期" }).focus();
  await user.keyboard("{ArrowRight}");
  await user.click(await screen.findByRole("menuitem", { name: "搁置" }));
  await waitFor(() => expect(row("没排的")).toHaveClass("shelved"));
  expect(within(row("没排的")).getByText("搁置")).toBeInTheDocument();
  expect(writes).toEqual([
    {
      method: "PUT",
      path: "/api/todos/没排的",
      body: {
        list_id: "work",
        title: "没排的",
        link: null,
        position: "a1",
        plan: { kind: "shelved" },
      },
    },
  ]);
});

// specs/test-cases/web/todo/todos.md#undoing-must-leave-the-server-as-if-nothing-happened
it("sends a deletion only once its notice is gone, and never after 撤销", async () => {
  const writes = serve();
  const { user } = mountApp("/todos/lists/work");
  await screen.findByText("没排的");
  await user.click(
    within(await menuOf("没排的", user)).getByRole("menuitem", {
      name: "删除",
    }),
  );
  await waitFor(() => expect(screen.queryByText("没排的")).toBeNull());
  await user.click(screen.getByRole("button", { name: "撤销" }));
  expect(await screen.findByText("没排的")).toBeInTheDocument();
  expect(writes).toEqual([]);

  // A newer notice settles the older one: the deletion goes out when the next change is shown,
  // which for a finish is once the server has taken it.
  await user.click(
    within(await menuOf("没排的", user)).getByRole("menuitem", {
      name: "删除",
    }),
  );
  await user.click(
    within(await menuOf("明天的", user)).getByRole("menuitem", {
      name: "完成",
    }),
  );
  await waitFor(() =>
    expect(writes).toEqual([
      { method: "POST", path: "/api/todos/明天的/complete", body: null },
      { method: "DELETE", path: "/api/todos/没排的", body: null },
    ]),
  );
  expect(await screen.findByText("已完成「明天的」")).toBeInTheDocument();
});
