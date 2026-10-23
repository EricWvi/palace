import {
  useState,
  type CSSProperties,
  type HTMLAttributes,
  type Ref,
} from "react";
import {
  bottomKey,
  inbox,
  isDoneToday,
  isToday,
  placeOf,
  samePlace,
  scheduleOf,
  tasksIn,
  tomorrowOf,
  topKey,
  whenLabel,
  type Place,
  type Todo,
} from "@/lib/todo";
import { MoreEntries, RightClickMenu, type MenuEntry } from "./todo-menu";
import { useTodoPage, type TodoPage } from "./todo-context";

// Hooks a row into sorting: the node, its moving style, and the pointer and key listeners.
export type DragBinding = {
  ref: Ref<HTMLLIElement>;
  style: CSSProperties;
  props: HTMLAttributes<HTMLLIElement>;
  dragging: boolean;
};

// The task menu: the legacy one without difficulty, the TimeTagger start and the board. The
// inbox is a buffer, so its tasks are neither planned nor finished.
function taskEntries(todo: Todo, page: TodoPage): MenuEntry[] {
  const { today, board, actions } = page;
  const schedule = scheduleOf(todo, today);
  const here = placeOf(todo);
  const item = (
    label: string,
    onSelect: () => void,
    extra = {},
  ): MenuEntry => ({
    kind: "item",
    label,
    onSelect,
    ...extra,
  });
  const planOn = (day: string) => () =>
    void actions.update(todo, { plan: { kind: "day", day } });
  const entries: MenuEntry[] = [];
  if (isToday(todo, today)) {
    const done = isDoneToday(todo, today);
    entries.push(
      item(
        done ? "撤销今日已做" : "今日已做",
        () => void actions.setDone(todo, today, !done),
      ),
    );
  }
  if (here.kind === "list") {
    const tomorrow = tomorrowOf(today);
    entries.push(
      item("完成", () => page.complete(todo)),
      {
        kind: "sub",
        label: "排期",
        entries: [
          item("今日", planOn(today), { current: isToday(todo, today) }),
          item("明日", planOn(tomorrow), {
            current: schedule.kind === "day" && schedule.day === tomorrow,
          }),
          item("选日期…", () => page.pickDate(todo)),
          { kind: "separator" },
          schedule.kind === "shelved"
            ? item(
                "取消搁置",
                () => void actions.update(todo, { plan: { kind: "open" } }),
              )
            : item(
                "搁置",
                () => void actions.update(todo, { plan: { kind: "shelved" } }),
              ),
          ...(schedule.kind === "day"
            ? [
                item(
                  "清除排期",
                  () => void actions.update(todo, { plan: { kind: "open" } }),
                ),
              ]
            : []),
        ],
      },
    );
  }
  const siblings = tasksIn(board, here).filter((t) => t.id !== todo.id);
  const places: { place: Place; name: string }[] = [
    { place: inbox, name: "收件箱" },
    ...board.lists.map((list) => ({
      place: { kind: "list", id: list.id } as Place,
      name: list.name,
    })),
  ];
  entries.push(
    { kind: "separator" },
    item("改名", () => page.setEditing({ id: todo.id, field: "title" })),
    item(todo.link ? "链接…" : "加链接…", () =>
      page.setEditing({ id: todo.id, field: "link" }),
    ),
    item(todo.has_body ? "正文" : "写正文", () => page.openBody(todo)),
    { kind: "separator" },
    item(
      "置顶",
      () => void actions.update(todo, { position: topKey(siblings) }),
    ),
    item(
      "置底",
      () => void actions.update(todo, { position: bottomKey(siblings) }),
    ),
    {
      kind: "sub",
      label: "移到",
      entries: places
        .filter(({ place }) => !samePlace(place, here))
        .map(({ place, name }) =>
          item(name, () => void actions.move(todo, place)),
        ),
    },
    { kind: "separator" },
    item("删除", () => page.remove(todo), { danger: true }),
  );
  return entries;
}

// Edits a title or a link where it is shown. Enter or leaving the field saves; Escape cancels.
function InlineEdit({
  initial,
  placeholder,
  onSave,
  onCancel,
}: {
  initial: string;
  placeholder?: string;
  onSave: (value: string) => void;
  onCancel: () => void;
}) {
  const [value, setValue] = useState(initial);
  // Escape and Enter both end editing, and the blur that follows must not save a second time.
  const [ended, setEnded] = useState(false);
  const end = (save: boolean) => {
    if (ended) return;
    setEnded(true);
    if (save) onSave(value.trim());
    else onCancel();
  };
  return (
    <input
      className="todo-input"
      autoFocus
      value={value}
      placeholder={placeholder}
      aria-label={placeholder ?? "任务标题"}
      onFocus={(event) => event.currentTarget.select()}
      onChange={(event) => setValue(event.target.value)}
      onBlur={() => end(true)}
      onPointerDown={(event) => event.stopPropagation()}
      onKeyDown={(event) => {
        // Typing in the field must neither drag the row nor reach the page's shortcuts.
        event.stopPropagation();
        if (event.nativeEvent.isComposing) return;
        if (event.key === "Enter") end(true);
        else if (event.key === "Escape") end(false);
      }}
    />
  );
}

/*
 * One task line. Today's rows lead with the done circle; list rows lead with a faint dot and say
 * when the task is planned. Right-click or the `···` opens the task menu.
 */
export function TodoRow({
  todo,
  context,
  drag,
}: {
  todo: Todo;
  context: "today" | "list";
  drag?: DragBinding;
}) {
  const page = useTodoPage();
  const { today, actions, editing, setEditing } = page;
  const done = context === "today" && isDoneToday(todo, today);
  const shelved = scheduleOf(todo, today).kind === "shelved";
  const when = context === "list" ? whenLabel(todo, today) : "";
  const editingTitle = editing?.id === todo.id && editing.field === "title";
  const editingLink = editing?.id === todo.id && editing.field === "link";
  const stop = () => setEditing(null);

  let title;
  if (editingTitle)
    title = (
      <InlineEdit
        initial={todo.title}
        onCancel={stop}
        onSave={(value) => {
          stop();
          if (value && value !== todo.title)
            void actions.update(todo, { title: value });
        }}
      />
    );
  else if (editingLink)
    title = (
      <InlineEdit
        initial={todo.link ?? ""}
        placeholder="链接，留空即移除"
        onCancel={stop}
        onSave={(value) => {
          stop();
          const link = value || null;
          if (link !== todo.link) void actions.update(todo, { link });
        }}
      />
    );
  else if (todo.link)
    title = (
      <a
        className="todo-title"
        href={todo.link}
        target="_blank"
        rel="noreferrer"
      >
        {todo.title}
      </a>
    );
  else title = <span className="todo-title">{todo.title}</span>;

  const className = [
    "todo-row",
    "reveals-more",
    done && "done",
    shelved && "shelved",
    drag?.dragging && "dragging",
  ]
    .filter(Boolean)
    .join(" ");
  const entries = taskEntries(todo, page);
  return (
    <RightClickMenu entries={entries}>
      <li
        ref={drag?.ref}
        style={drag?.style}
        {...drag?.props}
        className={className}
        data-id={todo.id}
        onDoubleClick={(event) => {
          if ((event.target as HTMLElement).closest(".todo-title"))
            setEditing({ id: todo.id, field: "title" });
        }}
      >
        {context === "today" ? (
          <button
            type="button"
            className="tick"
            role="checkbox"
            aria-checked={done}
            aria-label={`今日已做：${todo.title}`}
            onPointerDown={(event) => event.stopPropagation()}
            onClick={() => void actions.setDone(todo, today, !done)}
          />
        ) : (
          <span className="dash" aria-hidden="true">
            ·
          </span>
        )}
        {title}
        {!editingTitle && !editingLink && (
          <span className="todo-marks">
            {todo.link && <span aria-label="有链接">↗</span>}
            {todo.has_body && (
              <button
                type="button"
                onPointerDown={(event) => event.stopPropagation()}
                onClick={() => page.openBody(todo)}
              >
                正文
              </button>
            )}
          </span>
        )}
        <span className="todo-aside">
          {when && (
            <span className={when === "今日" ? "when today" : "when"}>
              {when}
            </span>
          )}
          <MoreEntries label={`${todo.title} 的操作`} entries={entries} />
        </span>
      </li>
    </RightClickMenu>
  );
}
