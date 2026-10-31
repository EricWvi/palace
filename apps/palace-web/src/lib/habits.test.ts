import { expect, it } from "vitest";
import {
  countsOf,
  fillOf,
  gridDays,
  nextCount,
  recentRate,
  type Habit,
} from "./habits";
import { dayParam } from "./day";

// Core test case: `specs/test-cases/web/habits/habits.md#tapping-a-seal-must-cycle-the-days-count`
it("next count wraps to zero after the day is full", () => {
  expect([0, 1, 2].map((count) => nextCount(count, 2))).toEqual([1, 2, 0]);
  expect(nextCount(0, 1)).toBe(1);
  expect(nextCount(1, 1)).toBe(0);
  // A day counted above a lowered times a day reads as full, so a tap clears it.
  expect(nextCount(3, 2)).toBe(0);
  expect([0, 1, 2, 5].map((count) => fillOf(count, 4))).toEqual([
    { kind: "none" },
    { kind: "part", fraction: 0.25 },
    { kind: "part", fraction: 0.5 },
    { kind: "done" },
  ]);
});

it("lays nine weeks from a Monday out to the last day", () => {
  // Saturday 2026-10-31.
  const days = gridDays(new Date(2026, 9, 31));
  expect([
    dayParam(days[0]),
    days[0].getDay(),
    dayParam(days.at(-1)!),
    days.length,
  ]).toEqual(["2026-08-31", 1, "2026-10-31", 62]);
});

it("rates the last 30 days, or the days since the habit began", () => {
  const habit = (created: Date): Habit => ({
    id: "h",
    name: "锻炼",
    mark: "锻",
    times_per_day: 2,
    created_at: created.getTime(),
    retired: false,
  });
  const counts = countsOf([
    { habit_id: "h", day: "2026-10-31", count: 2 },
    { habit_id: "h", day: "2026-10-30", count: 1 },
    { habit_id: "h", day: "2026-10-29", count: 3 },
  ]);
  const last = new Date(2026, 9, 31);
  // Born on the 29th at noon: three days, the 30th half done, the 29th capped at full.
  expect(
    recentRate(habit(new Date(2026, 9, 29, 12)), counts, last),
  ).toBeCloseTo(2.5 / 3);
  expect(recentRate(habit(new Date(2026, 0, 1)), counts, last)).toBeCloseTo(
    2.5 / 30,
  );
});
