import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
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
import { completedTodosOptions } from "@/lib/api";
import { keyAt, tasksIn, type Place, type Todo } from "@/lib/todo";
import { useTodoPage } from "./todo-context";
import { TodoRow } from "./todo-row";

function SortableRow({ todo }: { todo: Todo }) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: todo.id });
  return (
    <TodoRow
      todo={todo}
      context="list"
      drag={{
        ref: setNodeRef,
        // Rows only ever move up and down.
        style: {
          transform: CSS.Translate.toString(
            transform && { ...transform, x: 0 },
          ),
          transition,
        },
        // The row stays a list item; dnd-kit's own role would make it a button.
        props: { ...attributes, ...listeners, role: undefined },
        dragging: isDragging,
      }}
    />
  );
}

/*
 * A list's open tasks in key order, sortable by pointer or keyboard. The drop target is the row
 * nearest the pointer's centre, so a drag that overshoots the first or last row still lands at
 * that end. Dropping writes one key, between the new neighbours.
 */
function SortableTasks({ tasks }: { tasks: Todo[] }) {
  const { actions } = useTodoPage();
  const sensors = useSensors(
    // A few pixels of travel before a drag starts, so clicks, double-clicks and text selection
    // on a row still work.
    useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    }),
  );
  function drop({ active, over }: DragEndEvent) {
    if (!over || active.id === over.id) return;
    const moved = tasks.find((todo) => todo.id === active.id);
    const index = tasks.findIndex((todo) => todo.id === over.id);
    if (!moved || index < 0) return;
    void actions.update(moved, { position: keyAt(tasks, moved.id, index) });
  }
  return (
    <DndContext
      sensors={sensors}
      collisionDetection={closestCenter}
      onDragEnd={drop}
    >
      <SortableContext
        items={tasks.map((todo) => todo.id)}
        strategy={verticalListSortingStrategy}
      >
        <ul className="todo-rows">
          {tasks.map((todo) => (
            <SortableRow key={todo.id} todo={todo} />
          ))}
        </ul>
      </SortableContext>
    </DndContext>
  );
}

function stamp(time: number) {
  const date = new Date(time);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${date.getFullYear()}/${pad(date.getMonth() + 1)}/${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

// 已完成 under a list: folded to a count until opened; each can come back or go for good.
function Completed({ list }: { list: string }) {
  const { actions } = useTodoPage();
  const [open, setOpen] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const completed = useQuery(completedTodosOptions(list));
  const items = completed.data?.items ?? [];
  if (items.length === 0) return null;
  return (
    <section className="todo-completed" aria-label="已完成">
      <div className="todo-completed-head">
        <button
          type="button"
          aria-expanded={open}
          onClick={() => setOpen(!open)}
        >
          已完成 {items.length} {open ? "▴" : "▾"}
        </button>
        {open &&
          (confirming ? (
            <span className="clear">
              清空 {items.length} 个？
              <button type="button" onClick={() => setConfirming(false)}>
                取消
              </button>
              <button
                type="button"
                className="danger"
                onClick={() => {
                  setConfirming(false);
                  void actions.clearCompleted(list);
                }}
              >
                清空
              </button>
            </span>
          ) : (
            <button
              type="button"
              className="clear"
              onClick={() => setConfirming(true)}
            >
              清空
            </button>
          ))}
      </div>
      {open && (
        <ul className="todo-rows">
          {items.map((todo) => (
            <li key={todo.id} className="todo-row finished reveals-more">
              <span className="dash" aria-hidden="true">
                ·
              </span>
              {todo.link ? (
                <a
                  className="todo-title"
                  href={todo.link}
                  target="_blank"
                  rel="noreferrer"
                >
                  {todo.title}
                </a>
              ) : (
                <span className="todo-title">{todo.title}</span>
              )}
              <span className="todo-aside">
                {todo.done_days > 0 && <span>{todo.done_days} 天</span>}
                <span>{stamp(todo.completed_at)}</span>
                <button
                  type="button"
                  className="row-action"
                  onClick={() => void actions.restore(todo.id, list)}
                >
                  恢复
                </button>
                <button
                  type="button"
                  className="row-action"
                  onClick={() => void actions.remove(todo.id, list)}
                >
                  删除
                </button>
              </span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

// The inbox or one list: the add field when `n` opened it, the tasks, then 已完成.
export function ListView({
  place,
  title,
  adding,
  onAdded,
}: {
  place: Place;
  title: string;
  adding: boolean;
  onAdded: () => void;
}) {
  const { board, actions } = useTodoPage();
  const tasks = tasksIn(board, place);
  const [draft, setDraft] = useState("");
  return (
    <>
      <div className="todo-head">
        <h1>{title}</h1>
      </div>
      {adding && (
        <input
          className="todo-add"
          autoFocus
          aria-label="新任务"
          placeholder="新任务，回车添加"
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          onBlur={() => {
            if (!draft.trim()) onAdded();
          }}
          onKeyDown={(event) => {
            if (event.nativeEvent.isComposing) return;
            if (event.key === "Enter" && draft.trim()) {
              void actions.create(place, draft.trim());
              setDraft("");
            } else if (event.key === "Escape") {
              setDraft("");
              onAdded();
            }
          }}
        />
      )}
      {tasks.length > 0 ? (
        <SortableTasks tasks={tasks} />
      ) : (
        !adding && <p className="todo-empty">没有任务。</p>
      )}
      {place.kind === "list" && <Completed list={place.id} />}
    </>
  );
}
