import { expect, it } from "vitest";
import {
  dailySeries,
  defaultStart,
  parseLength,
  readActivity,
  smoothPath,
  typesByRecency,
  usualMinutes,
  type Activity,
} from "./activities";

const at = (day: number, hours: number, minutes = 0) =>
  new Date(2026, 9, day, hours, minutes);
const activity = (
  id: string,
  type: string,
  start: Date,
  minutes: number,
): Activity => ({
  id,
  type_id: type,
  type_name: type === "run" ? "跑步" : "力量",
  started_at: start.getTime(),
  minutes,
  note: null,
});

// Core test case: `specs/test-cases/web/habits/habits.md#recording-an-activity-must-read-the-hours-the-reader-typed`
it("reads an activity on the day it is filed under", () => {
  const now = at(31, 21, 30);
  expect([
    parseLength("35"),
    parseLength("1:30"),
    parseLength("1:75"),
    parseLength("x"),
  ]).toEqual([35, 90, null, null]);
  expect(readActivity(new Date(2026, 9, 28), "07:30", "1:30", now)).toEqual({
    kind: "activity",
    started_at: at(28, 7, 30).getTime(),
    minutes: 90,
  });
  expect(
    [
      readActivity(new Date(2026, 9, 31), "21:00", "45", now),
      readActivity(new Date(2026, 9, 31), "7点", "45", now),
      readActivity(new Date(2026, 9, 28), "07:00", "721", now),
      readActivity(new Date(2026, 9, 28), "07:00", "0", now),
    ].map((reading) =>
      reading.kind === "invalid" ? reading.field : reading.kind,
    ),
  ).toEqual(["length", "start", "length", "length"]);

  const runs = [
    activity("a", "run", at(20, 7), 30),
    activity("b", "run", at(21, 7), 43),
    activity("c", "lift", at(22, 20), 60),
  ];
  expect([
    usualMinutes(runs, "run"),
    usualMinutes(runs, "lift"),
    usualMinutes(runs, "yoga"),
  ]).toEqual([35, 60, 30]);
  expect(
    typesByRecency(
      [
        { id: "run", name: "跑步" },
        { id: "yoga", name: "瑜伽" },
        { id: "lift", name: "力量" },
      ],
      runs,
    ).map((type) => type.id),
  ).toEqual(["lift", "run", "yoga"]);
  // Today: just finished, rounded down to five minutes; another day: seven in the morning.
  expect(defaultStart(new Date(2026, 9, 31), 35, at(31, 21, 32))).toEqual(
    at(31, 20, 55),
  );
  expect(defaultStart(new Date(2026, 9, 28), 35, at(31, 21, 32))).toEqual(
    at(28, 7, 0),
  );
});

it("sums each type per day and orders types by their total", () => {
  const series = dailySeries(
    [
      activity("a", "run", at(30, 7), 30),
      activity("b", "run", at(30, 19), 15),
      activity("c", "lift", at(31, 20), 60),
      activity("d", "run", at(1, 7), 99),
    ],
    new Date(2026, 9, 31),
    3,
  );
  expect(
    series.map(({ type, minutes, times }) => [type.name, minutes, times]),
  ).toEqual([
    ["力量", [0, 0, 60], 1],
    ["跑步", [0, 45, 0], 2],
  ]);
});

// Core test case: `specs/test-cases/web/habits/habits.md#the-activity-curve-must-never-overshoot-its-days`
it("the curve stays between neighbouring days", () => {
  const values = [0, 30, 0, 45, 45, 0];
  const points = values.map((value, i): [number, number] => [
    i * 10,
    100 - value,
  ]);
  const path = smoothPath(points);
  const segments = path.split(" C").slice(1);
  expect(path.startsWith("M0,100")).toBe(true);
  expect(segments).toHaveLength(values.length - 1);
  segments.forEach((segment, i) => {
    const [c1, c2, end] = segment
      .split(" ")
      .map((pair) => pair.split(",").map(Number));
    const [low, high] = [
      Math.min(points[i][1], points[i + 1][1]),
      Math.max(points[i][1], points[i + 1][1]),
    ];
    for (const [, y] of [c1, c2]) {
      expect(y).toBeGreaterThanOrEqual(low);
      expect(y).toBeLessThanOrEqual(high);
    }
    expect(end).toEqual(points[i + 1]);
  });
});
