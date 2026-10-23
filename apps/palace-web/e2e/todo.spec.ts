import { expect, test, type Page } from "@playwright/test";
import type { Todo, TodoBoard } from "../src/lib/todo";

// One list of five tasks; every edit is applied to the board and recorded, as the server would.
function serve(page: Page) {
  const board: TodoBoard = {
    lists: [{ id: "life", name: "个人" }],
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
