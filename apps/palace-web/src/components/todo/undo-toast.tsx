import { useCallback, useEffect, useRef, useState } from "react";

// What a notice can do about the change it reports. `commit` runs when the notice goes away
// without an undo: that is when a deletion is really sent, so undoing one sends nothing.
type Notice = {
  text: string;
  undo?: () => void;
  commit?: () => void;
};

const SHOWN_MS = 4000;

// One line at the bottom of the page reporting the last change, with 撤销 when it can be undone.
// A newer notice, leaving the page or the timer commits the previous one first, so a deletion
// is never lost by being replaced.
export function useUndoToast() {
  const [notice, setNotice] = useState<Notice | null>(null);
  const pending = useRef<Notice | null>(null);
  const timer = useRef<number>(0);

  const settle = useCallback(() => {
    window.clearTimeout(timer.current);
    const settled = pending.current;
    pending.current = null;
    settled?.commit?.();
  }, []);

  const show = useCallback(
    (next: Notice) => {
      settle();
      pending.current = next;
      setNotice(next);
      timer.current = window.setTimeout(() => {
        settle();
        setNotice(null);
      }, SHOWN_MS);
    },
    [settle],
  );

  const undo = useCallback(() => {
    window.clearTimeout(timer.current);
    const undone = pending.current;
    pending.current = null;
    setNotice(null);
    undone?.undo?.();
  }, []);

  useEffect(() => settle, [settle]);

  const toast = notice && (
    <div className="todo-toast" role="status">
      <span>{notice.text}</span>
      {notice.undo && (
        <button type="button" onClick={undo}>
          撤销
        </button>
      )}
    </div>
  );
  return { show, toast };
}
