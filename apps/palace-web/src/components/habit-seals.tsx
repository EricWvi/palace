import type { CSSProperties } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, apiData, habitBoardOptions, type HabitBoard } from "@/lib/api";
import { dayParam } from "@/lib/day";
import {
  checkKey,
  countsOf,
  fillOf,
  nextCount,
  type Fill,
  type Habit,
} from "@/lib/habits";

type Tap = {
  habit: Habit;
  day: string;
  count: number;
  // The cached run the tap was made in, written first so the seal answers at once.
  queryKey: readonly unknown[];
};

/**
 * Sets a day's count for a habit. The cached run changes before the request leaves; a failed
 * write rereads every run. Taps on the same habit and day run one after another, so the last
 * one always lands last.
 */
export function useHabitTap() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: ({ habit, day, count }: Tap) =>
      apiData(
        api.PUT("/api/habits/{id}/checks/{day}", {
          params: { path: { id: habit.id, day } },
          body: { count },
        }),
      ),
    onMutate: ({ habit, day, count, queryKey }) => {
      client.setQueryData<HabitBoard>(queryKey, (board) => {
        if (!board) return board;
        const others = board.checks.filter(
          (check) => !(check.habit_id === habit.id && check.day === day),
        );
        return {
          ...board,
          checks:
            count === 0
              ? others
              : [...others, { habit_id: habit.id, day, count }],
        };
      });
    },
    onError: () => client.invalidateQueries({ queryKey: ["habits"] }),
    // Other runs holding this day are stale now; they refetch when next shown.
    onSuccess: () =>
      client.invalidateQueries({ queryKey: ["habits"], refetchType: "none" }),
    scope: { id: "habit-check" },
  });
}

// The inline style that fills a seal or cell from the bottom for a partly done day.
export function fillStyle(fill: Fill): CSSProperties | undefined {
  return fill.kind === "part"
    ? ({ "--fill": `${fill.fraction * 100}%` } as CSSProperties)
    : undefined;
}

/**
 * The day's 日课 as one-character seals on the date's line: empty until done, filled from the
 * bottom as a day's times add up, solid vermilion when done. A tap counts once more, and a full
 * day back to none; any day can be ticked, not only today.
 */
export function HabitSeals({ day }: { day: Date }) {
  const date = dayParam(day);
  const options = habitBoardOptions(date, date);
  const board = useQuery(options);
  const tap = useHabitTap();
  if (!board.data) return null;
  const counts = countsOf(board.data.checks);
  const habits = board.data.habits.filter((habit) => !habit.retired);
  if (habits.length === 0) return null;
  return (
    <div className="seals" role="group" aria-label="日课">
      {habits.map((habit) => {
        const count = counts.get(checkKey(habit.id, date)) ?? 0;
        const fill = fillOf(count, habit.times_per_day);
        const shown = Math.min(count, habit.times_per_day);
        const label =
          habit.times_per_day > 1
            ? `${habit.name} ${shown}/${habit.times_per_day}`
            : habit.name;
        return (
          <button
            key={habit.id}
            type="button"
            className="seal"
            data-fill={fill.kind}
            data-name={label}
            style={fillStyle(fill)}
            aria-label={label}
            aria-pressed={fill.kind === "done"}
            onClick={() =>
              tap.mutate({
                habit,
                day: date,
                count: nextCount(count, habit.times_per_day),
                queryKey: options.queryKey,
              })
            }
          >
            {habit.mark}
          </button>
        );
      })}
    </div>
  );
}
