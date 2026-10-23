import { useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import {
  isDoneToday,
  tasksIn,
  todayGroups,
  type Place,
  type TodoList,
} from "@/lib/todo";
import { Button } from "../ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../ui/dialog";
import { RightClickMenu } from "./todo-menu";
import { useTodoPage } from "./todo-context";

// What the left column points at: 今日, or a place.
export type View = { kind: "today" } | Place;

export function viewPath(view: View): string {
  if (view.kind === "today") return "/todos";
  if (view.kind === "inbox") return "/todos/inbox";
  return `/todos/lists/${view.id}`;
}

// A name typed where it is shown; Enter keeps it, Escape or an empty field gives up.
function NameField({
  initial,
  label,
  onDone,
}: {
  initial: string;
  label: string;
  onDone: (name: string | null) => void;
}) {
  const [name, setName] = useState(initial);
  const [ended, setEnded] = useState(false);
  const end = (keep: boolean) => {
    if (ended) return;
    setEnded(true);
    onDone(keep && name.trim() ? name.trim() : null);
  };
  return (
    <input
      className="todo-list-name"
      autoFocus
      aria-label={label}
      placeholder="集合名"
      value={name}
      onFocus={(event) => event.currentTarget.select()}
      onChange={(event) => setName(event.target.value)}
      onBlur={() => end(true)}
      onKeyDown={(event) => {
        event.stopPropagation();
        if (event.nativeEvent.isComposing) return;
        if (event.key === "Enter") end(true);
        else if (event.key === "Escape") end(false);
      }}
    />
  );
}

/*
 * The left column: 今日 with how much of it is done, then the inbox and the lists in creation
 * order with their open counts, and a ＋ that names a new list in place. Right-clicking a list
 * renames or deletes it; deleting asks first, since its tasks go with it.
 */
export function TodoSide({
  view,
  newList,
  onNewList,
}: {
  view: View;
  newList: boolean;
  onNewList: (open: boolean) => void;
}) {
  const { board, today, actions } = useTodoPage();
  const navigate = useNavigate();
  const [renaming, setRenaming] = useState<string | null>(null);
  const [deleting, setDeleting] = useState<TodoList | null>(null);
  const todayTasks = todayGroups(board, today).flatMap((group) => group.todos);
  const doneToday = todayTasks.filter((todo) =>
    isDoneToday(todo, today),
  ).length;
  const current = (target: View) =>
    (
      view.kind === "list" && target.kind === "list"
        ? view.id === target.id
        : view.kind === target.kind
    )
      ? "page"
      : undefined;
  const count = (place: Place) => tasksIn(board, place).length || "";

  return (
    <nav className="todo-side" aria-label="集合">
      <Link to="/todos" aria-current={current({ kind: "today" })}>
        今日
        <small>
          {todayTasks.length > 0 && `${doneToday}/${todayTasks.length}`}
        </small>
      </Link>
      <span className="gap" />
      <Link to="/todos/inbox" aria-current={current({ kind: "inbox" })}>
        收件箱<small>{count({ kind: "inbox" })}</small>
      </Link>
      {board.lists.map((list) =>
        renaming === list.id ? (
          <NameField
            key={list.id}
            initial={list.name}
            label="集合名"
            onDone={(name) => {
              setRenaming(null);
              if (name && name !== list.name)
                void actions.renameList(list.id, name);
            }}
          />
        ) : (
          <RightClickMenu
            key={list.id}
            entries={[
              {
                kind: "item",
                label: "改名",
                onSelect: () => setRenaming(list.id),
              },
              {
                kind: "item",
                label: "删除集合",
                danger: true,
                onSelect: () => setDeleting(list),
              },
            ]}
          >
            <Link
              to={viewPath({ kind: "list", id: list.id })}
              aria-current={current({ kind: "list", id: list.id })}
            >
              {list.name}
              <small>{count({ kind: "list", id: list.id })}</small>
            </Link>
          </RightClickMenu>
        ),
      )}
      {newList ? (
        <NameField
          initial=""
          label="新集合"
          onDone={async (name) => {
            onNewList(false);
            if (!name) return;
            const list = await actions.createList(name);
            if (list) navigate(viewPath({ kind: "list", id: list.id }));
          }}
        />
      ) : (
        <button
          type="button"
          className="add-list"
          aria-label="新建集合"
          onClick={() => onNewList(true)}
        >
          ＋
        </button>
      )}
      <Dialog
        open={deleting !== null}
        onOpenChange={(open) => {
          if (!open) setDeleting(null);
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>删除「{deleting?.name}」？</DialogTitle>
            <DialogDescription>
              集合里的任务，包括已完成的，都会一起删除，无法撤销。
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              type="button"
              variant="ghost"
              onClick={() => setDeleting(null)}
            >
              取消
            </Button>
            <Button
              onClick={() => {
                if (!deleting) return;
                if (current({ kind: "list", id: deleting.id }))
                  navigate("/todos");
                void actions.deleteList(deleting.id);
                setDeleting(null);
              }}
            >
              删除集合
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </nav>
  );
}
