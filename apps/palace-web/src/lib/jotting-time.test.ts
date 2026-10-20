import { expect, it } from "vitest";
import { absoluteTime, heatmapDays, relativeTime } from "./jotting-time";

it("words a time relatively inside seven days and not after", () => {
  const now = new Date(2026, 9, 20, 12, 0).getTime();
  const ago = (minutes: number) => relativeTime(now - minutes * 60_000, now);
  expect([
    ago(0.5),
    ago(5),
    ago(150),
    ago(60 * 24 * 3),
    ago(60 * 24 * 7),
  ]).toEqual(["刚刚", "5 分钟前", "2 小时前", "3 天前", null]);
  expect(absoluteTime(new Date(2026, 0, 2, 3, 4).getTime())).toBe(
    "2026-01-02 03:04",
  );
});

it("lays twelve weeks from a Monday through the current week's Sunday", () => {
  // Tuesday 2026-10-20: this week runs 10-19 to 10-25.
  const days = heatmapDays(new Date(2026, 9, 20, 9));
  expect([days.length, days[0], days[77], days[83]]).toEqual([
    84,
    "2026-08-03",
    "2026-10-19",
    "2026-10-25",
  ]);
  // A Sunday belongs to the week that began six days earlier.
  expect(heatmapDays(new Date(2026, 9, 25))[77]).toBe("2026-10-19");
});
