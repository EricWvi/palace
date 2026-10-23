import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { todoBodyOptions } from "@/lib/api";
import type { Todo } from "@/lib/todo";

// The field, once the body has arrived; it saves what changed when the panel closes.
function BodyField({
  initial,
  onClose,
}: {
  initial: string;
  onClose: (body: string | null) => void;
}) {
  const [text, setText] = useState(initial);
  const close = () => onClose(text === initial ? null : text);
  return (
    <>
      <div className="todo-panel-actions">
        <button type="button" onClick={close}>
          关闭
        </button>
      </div>
      <textarea
        autoFocus
        aria-label="正文"
        placeholder="写点什么"
        value={text}
        onChange={(event) => setText(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            event.stopPropagation();
            close();
          }
        }}
      />
    </>
  );
}

/*
 * A task's body in a panel sliding over the right of the page: plain Markdown in a text field,
 * saved when the panel closes (关闭 or Escape). An emptied body is removed.
 */
export function BodyPanel({
  todo,
  onClose,
}: {
  todo: Todo;
  // The changed text, or null when nothing changed.
  onClose: (body: string | null) => void;
}) {
  const body = useQuery(todoBodyOptions(todo.id));
  return (
    <aside className="todo-panel" aria-label={`${todo.title} 的正文`}>
      <div className="todo-panel-head">
        <h2>{todo.title}</h2>
        <span>正文</span>
      </div>
      {body.isError ? (
        <>
          <div className="todo-panel-actions">
            <button type="button" onClick={() => onClose(null)}>
              关闭
            </button>
          </div>
          <p className="todo-empty">正文读取失败，请稍后重试。</p>
        </>
      ) : body.data ? (
        <BodyField
          key={todo.id}
          initial={body.data.body ?? ""}
          onClose={onClose}
        />
      ) : null}
    </aside>
  );
}
