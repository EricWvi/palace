import { expect, test, type Page } from "@playwright/test";
import type { Todo, TodoBoard } from "../src/lib/todo";

// Three lists, the middle one of five tasks; every edit of a task or a list is applied to the
// board and recorded, as the server would.
function serve(page: Page) {
  const board: TodoBoard = {
    lists: [
      { id: "work", name: "工作", position: "a0" },
      { id: "life", name: "个人", position: "a1" },
      { id: "study", name: "学习", position: "a2" },
    ],
    todos: ["a0", "a1", "a2", "a3", "a4"].map((position, i): Todo => ({
      id: `t${i}`,
      list_id: "life",
      title: `任务 ${i}`,
      position,
      link: null,
      has_body: false,
      plan: { kind: "open" },
      done_days: 0,
    })),
  };
  const edits: { id: string; position: string }[] = [];
  void page.route("**/api/**", async (route) => {
    const request = route.request();
    const path = new URL(request.url()).pathname;
    if (path === "/api/todos") return route.fulfill({ json: board });
    if (path.endsWith("/completed"))
      return route.fulfill({ json: { items: [] } });
    const id = path.split("/")[3];
    const list = board.lists.find((l) => l.id === id);
    if (
      request.method() === "PUT" &&
      path.startsWith("/api/todo-lists/") &&
      list
    ) {
      const body = request.postDataJSON() as { name: string; position: string };
      edits.push({ id, position: body.position });
      Object.assign(list, body);
      return route.fulfill({ json: list });
    }
    const todo = board.todos.find((t) => t.id === id);
    if (request.method() === "PUT" && todo) {
      const body = request.postDataJSON() as { position: string };
      edits.push({ id, position: body.position });
      todo.position = body.position;
      return route.fulfill({ json: todo });
    }
    return route.fulfill({ status: 404, json: { error: "not_found" } });
  });
  return edits;
}

const titles = (page: Page) =>
  page.locator(".todo-main > .todo-rows .todo-title").allTextContents();

// Drags a row by the pointer to a height on the page, well past where it starts.
async function drag(page: Page, title: string, toY: number) {
  const box = (await page.getByText(title).boundingBox())!;
  const x = box.x + 40;
  await page.mouse.move(x, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(x, box.y + box.height / 2 + 8, { steps: 4 });
  await page.mouse.move(x, toY, { steps: 12 });
  await page.mouse.up();
}

// specs/test-cases/web/todo/todos.md#task-order-must-come-from-unique-well-formed-keys
test("dragging past either end lands there and writes only the moved task's key", async ({
  page,
}) => {
  const edits = serve(page);
  await page.goto("/todos/lists/life");
  await expect(page.getByText("任务 4")).toBeVisible();

  await drag(page, "任务 3", 0);
  await expect
    .poll(() => titles(page))
    .toEqual(["任务 3", "任务 0", "任务 1", "任务 2", "任务 4"]);
  await drag(page, "任务 0", page.viewportSize()!.height - 1);
  await expect
    .poll(() => titles(page))
    .toEqual(["任务 3", "任务 1", "任务 2", "任务 4", "任务 0"]);
  // Two moves, two writes, each of one task with a key beyond the end it went to.
  expect(edits.map((edit) => edit.id)).toEqual(["t3", "t0"]);
  const [top, bottom] = edits.map((edit) => edit.position);
  expect(top < "a0").toBe(true);
  expect(bottom > "a4").toBe(true);
});

// specs/test-cases/web/todo/todos.md#task-order-must-come-from-unique-well-formed-keys
test("the keyboard carries a row and writes a key between its new neighbours", async ({
  page,
}) => {
  const edits = serve(page);
  await page.goto("/todos/lists/life");
  await expect(page.getByText("任务 4")).toBeVisible();
  // Each step waits for dnd-kit's own announcement, as a reader would see the row move.
  const row = page.locator("li", { hasText: "任务 1" });
  await row.focus();
  await page.keyboard.press("Space");
  await expect(row).toHaveClass(/dragging/);
  await page.keyboard.press("ArrowDown");
  await expect(
    page.getByText(/was moved over droppable area t2/),
  ).toBeAttached();
  await page.keyboard.press("Space");
  await expect
    .poll(() => titles(page))
    .toEqual(["任务 0", "任务 2", "任务 1", "任务 3", "任务 4"]);
  expect(edits.map((edit) => edit.id)).toEqual(["t1"]);
  expect("a2" < edits[0].position && edits[0].position < "a3").toBe(true);
});

const sideNames = (page: Page) =>
  page
    .getByRole("navigation", { name: "集合" })
    .getByRole("link")
    .evaluateAll((links) => links.map((link) => link.firstChild?.textContent));

// specs/test-cases/web/todo/todos.md#list-order-must-come-from-unique-well-formed-keys
test("lists drag below the fixed inbox, and a drop neither opens the list nor writes another", async ({
  page,
}) => {
  const edits = serve(page);
  await page.goto("/todos/lists/life");
  await expect(page.getByText("任务 4")).toBeVisible();

  // Far above the first list: it lands first, still under the inbox.
  await drag(page, "学习", 0);
  await expect
    .poll(() => sideNames(page))
    .toEqual(["今日", "收件箱", "学习", "工作", "个人"]);
  await expect(page).toHaveURL(/\/todos\/lists\/life$/);
  expect(edits.map((edit) => edit.id)).toEqual(["study"]);
  expect(edits[0].position < "a0").toBe(true);

  // The keyboard: Space picks 个人 up, one step up, Space drops it between 学习 and 工作.
  const life = page.getByRole("link", { name: /^个人/ });
  await life.focus();
  await page.keyboard.press("Space");
  await expect(life).toHaveClass(/dragging/);
  // dnd-kit listens for the arrow keys from a timer it sets on pick-up; a timer set now runs after
  // it, so the next key is heard.
  await page.evaluate(() => new Promise((resolve) => setTimeout(resolve)));
  await page.keyboard.press("ArrowUp");
  await expect(
    page.getByText(/was moved over droppable area work/),
  ).toBeAttached();
  await page.keyboard.press("Space");
  await expect
    .poll(() => sideNames(page))
    .toEqual(["今日", "收件箱", "学习", "个人", "工作"]);
  expect(edits.map((edit) => edit.id)).toEqual(["study", "life"]);
  expect(edits[0].position < edits[1].position).toBe(true);
  expect(edits[1].position < "a0").toBe(true);

  // Enter still opens a list.
  await page.getByRole("link", { name: /^工作/ }).focus();
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/\/todos\/lists\/work$/);
});
