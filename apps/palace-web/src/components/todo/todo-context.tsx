import { createContext, useContext } from "react";
import type { Todo, TodoBoard } from "@/lib/todo";
import type { TodoActions } from "./use-todo-actions";

// Which text of which task is being edited in place.
export type Editing = { id: string; field: "title" | "link" } | null;

// What every row, menu and view of 行事 shares: the reader's today, the board, the actions,
// and the page-level steps that need more than the row (a dialog, the body panel, the undo line).
export type TodoPage = {
  today: string;
  board: TodoBoard;
  actions: TodoActions;
  editing: Editing;
  setEditing: (editing: Editing) => void;
  openBody: (todo: Todo) => void;
  pickDate: (todo: Todo) => void;
  complete: (todo: Todo) => void;
  remove: (todo: Todo) => void;
};

export const TodoPageContext = createContext<TodoPage | null>(null);

export function useTodoPage(): TodoPage {
  const page = useContext(TodoPageContext);
  if (!page) throw new Error("useTodoPage outside the 行事 page");
  return page;
}
