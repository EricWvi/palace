import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate, useParams } from "react-router-dom";
import { todoBoardOptions } from "@/lib/api";
import { dayParam, parseDay } from "@/lib/day";
import { inbox, type Place, type Todo } from "@/lib/todo";
import { BodyPanel } from "@/components/todo/body-panel";
import { ListView } from "@/components/todo/list-view";
import { TodayView } from "@/components/todo/today-view";
import {
  TodoPageContext,
  type Editing,
  type TodoPage as Page,
} from "@/components/todo/todo-context";
import { TodoSide, viewPath, type View } from "@/components/todo/todo-side";
import { useUndoToast } from "@/components/todo/undo-toast";
import { useTodoActions } from "@/components/todo/use-todo-actions";
import { DayCalendar } from "@/components/day-calendar";
import { ErrorState } from "@/components/error-state";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  useShortcutHelp,
  type ShortcutGroup,
} from "@/components/shortcut-help";

// 行事: 今日 and every list in a left column, the open one on the right. `n` adds a task and `p`
// plans the day; neither has a visible entry besides 安排.
// See specs/decisions/web/todo/0-today-and-lists-with-fractional-order.md.
const shortcuts: ShortcutGroup = {
  title: "行事",
  shortcuts: [
    { keys: ["n"], action: "新建任务（在今日时放进收件箱）" },
    { keys: ["p"], action: "安排今日" },
    { keys: ["Esc"], action: "取消安排或新建" },
  ],
};

// The reader's local today, `YYYY-MM-DD`. A page left open past midnight moves on when it is
// looked at again, and once a minute while it is in view.
function useToday(): string {
  const [today, setToday] = useState(() => dayParam(new Date()));
  useEffect(() => {
    const check = () => setToday(dayParam(new Date()));
    const timer = window.setInterval(check, 60_000);
    document.addEventListener("visibilitychange", check);
    window.addEventListener("focus", check);
    return () => {
      window.clearInterval(timer);
      document.removeEventListener("visibilitychange", check);
      window.removeEventListener("focus", check);
    };
  }, []);
  return today;
}

export function TodoPage({ view }: { view: "today" | "inbox" | "list" }) {
  const { id } = useParams();
  const navigate = useNavigate();
  const today = useToday();
  const board = useQuery(todoBoardOptions());
  const { show, toast } = useUndoToast();
  const actions = useTodoActions((message) => show({ text: message }));
  const [planning, setPlanning] = useState(false);
  const [picked, setPicked] = useState<Set<string>>(new Set());
  // The view whose add field `n` opened; it can open before the view it belongs to is shown.
  const [addingIn, setAddingIn] = useState<string | null>(null);
  const [newList, setNewList] = useState(false);
  const [editing, setEditing] = useState<Editing>(null);
  const [bodyOf, setBodyOf] = useState<string | null>(null);
  const [dating, setDating] = useState<Todo | null>(null);

  const current: View =
    view === "today"
      ? { kind: "today" }
      : view === "inbox"
        ? inbox
        : { kind: "list", id: id ?? "" };
  const adding = addingIn === viewPath(current);
  // Planning belongs to 今日: leaving 今日 drops a half-made plan, while `p` elsewhere first
  // comes to 今日 and opens it there.
  const [shown, setShown] = useState(viewPath(current));
  if (shown !== viewPath(current)) {
    setShown(viewPath(current));
    if (shown === viewPath({ kind: "today" })) setPlanning(false);
  }

  function startPlanning() {
    setPicked(new Set());
    setAddingIn(null);
    if (current.kind !== "today") navigate("/todos");
    setPlanning(true);
  }

  useShortcutHelp(shortcuts);
  useEffect(() => {
    function key(event: KeyboardEvent) {
      if (event.metaKey || event.ctrlKey || event.altKey) return;
      const target = event.target as HTMLElement;
      if (
        target.closest(
          "input, textarea, select, [contenteditable], [role=dialog], [role=menu]",
        )
      )
        return;
      if (event.key === "Escape") {
        if (bodyOf) setBodyOf(null);
        else if (planning) setPlanning(false);
        else if (adding) setAddingIn(null);
      } else if (event.key === "n") {
        event.preventDefault();
        setPlanning(false);
        // 今日 shows no list to add to; a new task waits in the inbox.
        if (current.kind === "today") {
          setAddingIn(viewPath(inbox));
          navigate(viewPath(inbox));
        } else setAddingIn(viewPath(current));
      } else if (event.key === "p") {
        event.preventDefault();
        startPlanning();
      }
    }
    document.addEventListener("keydown", key);
    return () => document.removeEventListener("keydown", key);
  });

  if (board.isError) return <ErrorState error={board.error} />;
  if (!board.data) return null;
  const data = board.data;
  const list =
    current.kind === "list"
      ? data.lists.find((l) => l.id === current.id)
      : undefined;
  if (current.kind === "list" && !list)
    return <p className="quiet-note">这个集合不存在了。</p>;

  const page: Page = {
    today,
    board: data,
    actions,
    editing,
    setEditing,
    openBody: (todo) => setBodyOf(todo.id),
    pickDate: setDating,
    complete: (todo) => {
      void actions.complete(todo).then((finished) => {
        if (finished)
          show({
            text: `已完成「${todo.title}」`,
            undo: () => void actions.undoComplete(todo),
          });
      });
    },
    // The row goes at once, but the deletion is sent only when the notice goes away, so 撤销
    // simply puts the row back.
    remove: (todo) => {
      actions.hide(todo.id);
      show({
        text: `已删除「${todo.title}」`,
        undo: () => actions.reinsert(todo),
        commit: () => void actions.remove(todo.id, todo.list_id),
      });
    },
  };
  const bodyTodo = bodyOf
    ? data.todos.find((todo) => todo.id === bodyOf)
    : undefined;
  const place: Place | null = current.kind === "today" ? null : current;

  return (
    <TodoPageContext value={page}>
      <div className="todo">
        <TodoSide view={current} newList={newList} onNewList={setNewList} />
        <div className="todo-main">
          {place ? (
            <ListView
              key={viewPath(place)}
              place={place}
              title={list?.name ?? "收件箱"}
              adding={adding}
              onAdded={() => setAddingIn(null)}
            />
          ) : (
            <TodayView
              planning={planning}
              picked={picked}
              onPick={(id) => {
                const next = new Set(picked);
                if (!next.delete(id)) next.add(id);
                setPicked(next);
              }}
              onStart={startPlanning}
              onCancel={() => setPlanning(false)}
              onPlan={() => {
                setPlanning(false);
                void actions.plan(today, [...picked]);
              }}
            />
          )}
        </div>
      </div>
      {bodyTodo && (
        <BodyPanel
          todo={bodyTodo}
          onClose={(body) => {
            setBodyOf(null);
            if (body !== null)
              void actions.setBody(bodyTodo, body.trim() ? body : null);
          }}
        />
      )}
      <Dialog
        open={dating !== null}
        onOpenChange={(open) => {
          if (!open) setDating(null);
        }}
      >
        <DialogContent className="todo-date" aria-describedby={undefined}>
          <DialogHeader>
            <DialogTitle>排到哪一天</DialogTitle>
          </DialogHeader>
          {dating && (
            <DayCalendar
              selected={
                (dating.plan.kind === "day" && parseDay(dating.plan.day)) ||
                parseDay(today) ||
                new Date()
              }
              onSelect={(day) => {
                void actions.update(dating, {
                  plan: { kind: "day", day: dayParam(day) },
                });
                setDating(null);
              }}
            />
          )}
        </DialogContent>
      </Dialog>
      {toast}
    </TodoPageContext>
  );
}
