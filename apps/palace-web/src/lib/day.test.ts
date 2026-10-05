import { expect, it } from "vitest";
import {
  addDays,
  atCurrentTime,
  backLabel,
  dayHeading,
  dayOf,
  dayParam,
  dayRange,
  parseDay,
} from "./day";

it("reads only real calendar days and writes them back unchanged", () => {
  const day = parseDay("2025-09-30")!;
  expect([dayParam(day), dayHeading(day), backLabel(day)]).toEqual([
    "2025-09-30",
    "Sep 30, 2025",
    "← 9 月 30 日",
  ]);
  for (const value of [null, "", "2025-9-30", "2025-02-30", "2025-13-01", "x"])
    expect(parseDay(value)).toBeNull();
  expect(dayParam(parseDay("0025-01-01")!)).toBe("0025-01-01");
});

it("bounds a day by local midnights and keeps the time of day for new records", () => {
  const day = parseDay("2025-03-09")!;
  const { start, end } = dayRange(day);
  expect([start, end]).toEqual([day.getTime(), addDays(day, 1).getTime()]);
  expect(dayOf(end - 1)).toEqual(day);
  expect(dayOf(end)).toEqual(addDays(day, 1));
  const now = new Date();
  const record = atCurrentTime(day);
  expect([dayParam(record), record.getHours(), record.getMinutes()]).toEqual([
    "2025-03-09",
    now.getHours(),
    now.getMinutes(),
  ]);
});
