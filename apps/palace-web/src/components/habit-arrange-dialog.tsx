import { useState, type FormEvent } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { api, apiData } from "@/lib/api";
import type { Habit } from "@/lib/habits";
import { ErrorState } from "./error-state";
import { Button } from "./ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "./ui/dialog";

// One row of the form: a habit being edited, or one that does not exist yet.
type Row = {
  key: string;
  id: string | null;
  name: string;
  mark: string;
  times: number;
};

const TIMES = [...Array(10).keys()].map((index) => index + 1);

// The seal's character: what was typed, or else the name's first character.
function markOf(row: Row): string {
  return [...row.mark.trim()][0] ?? [...row.name.trim()][0] ?? "";
}

/**
 * 编排日课: every habit in use as a row of name, seal character and times a day, each retirable,
 * and new ones added at the bottom. Saving writes only what changed; a row left without a name
 * is dropped.
 */
export function HabitArrangeDialog({
  habits,
  onClose,
}: {
  habits: Habit[];
  onClose: () => void;
}) {
  const client = useQueryClient();
  const [rows, setRows] = useState<Row[]>(() =>
    habits.map((habit) => ({
      key: habit.id,
      id: habit.id,
      name: habit.name,
      mark: habit.mark,
      times: habit.times_per_day,
    })),
  );
  const [added, setAdded] = useState(0);

  const save = useMutation({
    mutationFn: async () => {
      const kept = new Set(rows.map((row) => row.id));
      const writes: Promise<unknown>[] = habits
        .filter((habit) => !kept.has(habit.id))
        .map((habit) =>
          apiData(
            api.POST("/api/habits/{id}/retire", {
              params: { path: { id: habit.id } },
            }),
          ),
        );
      for (const row of rows) {
        const body = {
          name: row.name.trim(),
          mark: markOf(row),
          times_per_day: row.times,
        };
        if (!body.name) continue;
        const was = habits.find((habit) => habit.id === row.id);
        if (!row.id) writes.push(apiData(api.POST("/api/habits", { body })));
        else if (
          was &&
          (was.name !== body.name ||
            was.mark !== body.mark ||
            was.times_per_day !== body.times_per_day)
        )
          writes.push(
            apiData(
              api.PUT("/api/habits/{id}", {
                params: { path: { id: row.id } },
                body,
              }),
            ),
          );
      }
      // The order does not matter: each write touches its own habit.
      await Promise.all(writes);
    },
    onSettled: () => client.invalidateQueries({ queryKey: ["habits"] }),
    onSuccess: onClose,
  });

  const update = (key: string, change: Partial<Row>) =>
    setRows((current) =>
      current.map((row) => (row.key === key ? { ...row, ...change } : row)),
    );

  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open && !save.isPending) onClose();
      }}
    >
      <DialogContent className="habit-dialog">
        <DialogHeader>
          <DialogTitle>编排日课</DialogTitle>
          <DialogDescription className="sr-only">
            修改日课的名称、印章字和每天次数，或停用
          </DialogDescription>
        </DialogHeader>
        <form
          onSubmit={(event: FormEvent) => {
            event.preventDefault();
            save.mutate();
          }}
        >
          <fieldset disabled={save.isPending} className="habit-rows">
            {rows.map((row, index) => (
              <div className="habit-row" key={row.key}>
                <input
                  className="habit-name"
                  aria-label={`第 ${index + 1} 门的名称`}
                  placeholder="名称"
                  value={row.name}
                  autoFocus={row.id === null && index === rows.length - 1}
                  onChange={(event) =>
                    update(row.key, { name: event.target.value })
                  }
                />
                <input
                  className="habit-mark"
                  aria-label={`第 ${index + 1} 门的印章字`}
                  placeholder={[...row.name.trim()][0] ?? "字"}
                  value={row.mark}
                  onChange={(event) =>
                    update(row.key, {
                      mark: [...event.target.value.trim()].slice(-1).join(""),
                    })
                  }
                />
                <select
                  className="habit-times"
                  aria-label={`第 ${index + 1} 门的每天次数`}
                  value={row.times}
                  onChange={(event) =>
                    update(row.key, { times: Number(event.target.value) })
                  }
                >
                  {TIMES.map((times) => (
                    <option key={times} value={times}>
                      每天 {times} 次
                    </option>
                  ))}
                </select>
                <button
                  type="button"
                  className="habit-retire"
                  onClick={() =>
                    setRows((current) =>
                      current.filter((other) => other.key !== row.key),
                    )
                  }
                >
                  {row.id ? "停用" : "移除"}
                </button>
              </div>
            ))}
            <button
              type="button"
              className="habit-add"
              onClick={() => {
                setRows((current) => [
                  ...current,
                  {
                    key: `new-${added}`,
                    id: null,
                    name: "",
                    mark: "",
                    times: 1,
                  },
                ]);
                setAdded(added + 1);
              }}
            >
              ＋ 新增一门
            </button>
          </fieldset>
          <p className="habit-hint">
            停用的日课不再出现在每天的页面上，记录留在历史里。
          </p>
          {save.isError && <ErrorState error={save.error} />}
          <DialogFooter className="sleep-actions">
            <span className="spacer" />
            <Button
              type="button"
              variant="ghost"
              disabled={save.isPending}
              onClick={onClose}
            >
              取消
            </Button>
            <Button type="submit" disabled={save.isPending}>
              {save.isPending ? "正在保存…" : "保存"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
