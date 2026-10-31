// 时刻's 日课: how a seal reads a day's count, what a tap sets it to, and the history's grid. A
// check has no time; the day is always the browser's local day.
import { addDays, dayParam } from "./day";

export type Habit = {
  id: string;
  name: string;
  mark: string;
  times_per_day: number;
  created_at: number;
  retired: boolean;
};
export type HabitCheck = { habit_id: string; day: string; count: number };

// The count a tap sets: one more, and after a full day back to none, so a slip is undone by
// tapping on. A count above a lowered times a day reads as full, so the next tap clears it.
export function nextCount(count: number, times: number): number {
  return Math.min(count, times) >= times ? 0 : count + 1;
}

// How a seal or a grid cell looks for a count: empty, filled to a fraction, or done.
export type Fill =
  { kind: "none" } | { kind: "part"; fraction: number } | { kind: "done" };

export function fillOf(count: number, times: number): Fill {
  if (count <= 0) return { kind: "none" };
  if (count >= times) return { kind: "done" };
  return { kind: "part", fraction: count / times };
}

// Every check of a run of days by habit and day, so a seal or a cell finds its count at once.
export function countsOf(checks: HabitCheck[]): Map<string, number> {
  return new Map(
    checks.map((check) => [checkKey(check.habit_id, check.day), check.count]),
  );
}

export function checkKey(habit: string, day: string): string {
  return `${habit}|${day}`;
}

// The history's grid: nine weeks from the Monday eight weeks before this week's, up to `last`.
export function gridDays(last: Date): Date[] {
  const monday = addDays(last, -((last.getDay() + 6) % 7));
  const first = addDays(monday, -56);
  const days: Date[] = [];
  for (let day = first; day <= last; day = addDays(day, 1)) days.push(day);
  return days;
}

// The share of the last 30 days (fewer if the habit is younger) a habit was done, each day
// counting its fraction of the times a day.
export function recentRate(
  habit: Habit,
  counts: Map<string, number>,
  last: Date,
): number {
  const born = new Date(habit.created_at);
  const bornDay = new Date(born.getFullYear(), born.getMonth(), born.getDate());
  let days = 0;
  let done = 0;
  for (let back = 0; back < 30; back++) {
    const day = addDays(last, -back);
    if (day < bornDay) break;
    days++;
    const count = counts.get(checkKey(habit.id, dayParam(day))) ?? 0;
    done += Math.min(count, habit.times_per_day) / habit.times_per_day;
  }
  return days === 0 ? 0 : done / days;
}
