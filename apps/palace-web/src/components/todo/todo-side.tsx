import { useEffect, useState, type ReactNode } from "react";
import { Link, useNavigate } from "react-router-dom";
import {
  DndContext,
  KeyboardSensor,
  PointerSensor,
  closestCenter,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import {
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import {
  isDoneToday,
  keyAt,
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
import { RightClickMenu, type MenuEntry } from "./todo-menu";
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

function cancelClick(event: MouseEvent) {
  event.preventDefault();
}

// One list's link, moving with the pointer or the keyboard while it is dragged. The link itself
// is the menu's trigger, so the menu can hand it its handlers and ref.
function SortableLink({
  list,
  current,
  entries,
  children,
}: {
  list: TodoList;
  current: "page" | undefined;
  entries: MenuEntry[];
  children: ReactNode;
}) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: list.id });
  return (
    <RightClickMenu entries={entries}>
      <Link
        ref={setNodeRef}
        className={isDragging ? "dragging" : undefined}
        to={viewPath({ kind: "list", id: list.id })}
        aria-current={current}
        // Lists only ever move up and down.
        style={{
          transform: CSS.Translate.toString(
            transform && { ...transform, x: 0 },
          ),
          transition,
        }}
        {...attributes}
        {...listeners}
        // It stays a link; dnd-kit's own role would make it a button.
        role={undefined}
      >
        {children}
      </Link>
    </RightClickMenu>
  );
}

/*
 * The left column: 今日 with how much of it is done, then the inbox and the lists in their
 * dragged order with their open counts, and a ＋ that names a new list in place. The inbox stays
 * on top and does not move. Right-clicking a list renames or deletes it; deleting asks first,
 * since its tasks go with it.
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
  const sensors = useSensors(
    // A few pixels of travel before a drag starts, so a click still opens the list.
    useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
    // Space alone picks a list up: Enter on a link must keep opening it.
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
      keyboardCodes: {
        start: ["Space"],
        cancel: ["Escape"],
        end: ["Space", "Enter"],
      },
    }),
  );
  // Releasing a dragged link still clicks it. dnd-kit stops that click's propagation from the
  // start of a drag until 50ms after its end, so React never sees it, but the browser would still
  // follow the href and reload the page. Over the same window its default is cancelled too.
  const holdClicks = () =>
    document.addEventListener("click", cancelClick, true);
  const releaseClicks = () =>
    window.setTimeout(
      () => document.removeEventListener("click", cancelClick, true),
      50,
    );
  useEffect(
    () => () => document.removeEventListener("click", cancelClick, true),
    [],
  );
  function drop({ active, over }: DragEndEvent) {
    if (!over || active.id === over.id) return;
    const moved = board.lists.find((list) => list.id === active.id);
    const index = board.lists.findIndex((list) => list.id === over.id);
    if (!moved || index < 0) return;
    void actions.updateList(moved, {
      position: keyAt(board.lists, moved.id, index),
    });
  }

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
      <DndContext
        sensors={sensors}
        collisionDetection={closestCenter}
        onDragStart={holdClicks}
        onDragEnd={(event) => {
          releaseClicks();
          drop(event);
        }}
        onDragCancel={releaseClicks}
      >
        <SortableContext
          items={board.lists.map((list) => list.id)}
          strategy={verticalListSortingStrategy}
        >
          {board.lists.map((list) =>
            renaming === list.id ? (
              <NameField
                key={list.id}
                initial={list.name}
                label="集合名"
                onDone={(name) => {
                  setRenaming(null);
                  if (name && name !== list.name)
                    void actions.updateList(list, { name });
                }}
              />
            ) : (
              <SortableLink
                key={list.id}
                list={list}
                current={current({ kind: "list", id: list.id })}
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
                {list.name}
                <small>{count({ kind: "list", id: list.id })}</small>
              </SortableLink>
            ),
          )}
        </SortableContext>
      </DndContext>
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
