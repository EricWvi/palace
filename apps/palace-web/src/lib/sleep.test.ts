import { expect, it } from "vitest";
import {
  asksBeforeEnding,
  averageClock,
  axisPercent,
  expired,
  longDuration,
  nightOn,
  readNight,
  sleepAction,
  stopwatch,
  type Night,
} from "./sleep";

// Local times, so the rules read the same in any time zone the tests run in.
const at = (day: number, hours: number, minutes = 0) =>
  new Date(2026, 9, day, hours, minutes);

// Core test case: `specs/test-cases/web/sleep/sleep.md#the-menu-must-offer-sleep-by-the-readers-clock`
it("offers starting in the evening and night, recording in the morning, nothing while running", () => {
  const offer = (
    hours: number,
    state = { running: false, sleptToday: false },
  ) => sleepAction(at(29, hours), state);
  expect([
    offer(17),
    offer(18),
    offer(23),
    offer(4),
    offer(5),
    offer(14),
    offer(15),
    offer(9, { running: false, sleptToday: true }),
    offer(23, { running: true, sleptToday: false }),
    offer(9, { running: true, sleptToday: false }),
  ]).toEqual([
    null,
    "start",
    "start",
    "start",
    "record",
    "record",
    null,
    null,
    null,
    null,
  ]);
});

// Core test case: `specs/test-cases/web/sleep/sleep.md#a-forgotten-start-must-lapse-at-three-the-next-afternoon`
it("lets a start lapse at 15:00 of the day after its evening", () => {
  const evening = at(28, 23, 46).getTime();
  const smallHours = at(29, 1, 30).getTime();
  expect([
    expired(evening, at(29, 14, 59)),
    expired(evening, at(29, 15)),
    expired(smallHours, at(29, 14, 59)),
    expired(smallHours, at(29, 15)),
  ]).toEqual([false, true, false, true]);
});

// Core test case: `specs/test-cases/web/sleep/sleep.md#ending-before-six-must-ask-once-more`
it("asks before ending from the evening until 06:00", () => {
  expect(
    [1, 5, 6, 11, 17, 18, 23].map((h) => asksBeforeEnding(at(29, h))),
  ).toEqual([true, true, false, false, false, true, true]);
});

// Core test case: `specs/test-cases/web/sleep/sleep.md#a-typed-night-must-land-on-its-wake-day`
it("reads a typed night onto its wake day and names what does not add up", () => {
  const day = new Date(2026, 9, 29);
  const now = at(29, 9);
  expect([
    readNight(day, "23:30", "7:05", now),
    readNight(day, "0130", "07:05", now),
    readNight(day, "23:30", "10:30", now),
    readNight(day, "07:30", "07:05", now),
    readNight(day, "12:00", "07:00", now),
    readNight(day, "23:3", "07:00", now),
  ]).toEqual([
    {
      kind: "night",
      started_at: at(28, 23, 30).getTime(),
      ended_at: at(29, 7, 5).getTime(),
    },
    {
      kind: "night",
      started_at: at(29, 1, 30).getTime(),
      ended_at: at(29, 7, 5).getTime(),
    },
    { kind: "invalid", reason: "醒来时间还没到", field: "wake" },
    { kind: "invalid", reason: "醒来要晚于入睡", field: "wake" },
    { kind: "invalid", reason: "超过 16 小时，检查一下入睡时间", field: "bed" },
    { kind: "invalid", reason: "按 23:30 这样的格式填写", field: "bed" },
  ]);
});

it("files nights under their wake day and averages bedtimes across midnight", () => {
  const night = (id: string, bed: Date, wake: Date): Night => ({
    id,
    started_at: bed.getTime(),
    ended_at: wake.getTime(),
  });
  const nights = [
    night("a", at(27, 23, 30), at(28, 7)),
    night("b", at(29, 0, 30), at(29, 7, 30)),
  ];
  expect([
    nightOn(nights, new Date(2026, 9, 29))?.id,
    nightOn(nights, new Date(2026, 9, 27)),
    averageClock(nights, "started_at"),
    averageClock(nights, "ended_at"),
    averageClock([], "ended_at"),
    longDuration(446),
    stopwatch(4_984_000),
    axisPercent(new Date(2026, 9, 29), at(29, 3).getTime()),
  ]).toEqual([
    "b",
    undefined,
    "00:00",
    "07:15",
    null,
    "7 小时 26 分",
    "1:23:04",
    50,
  ]);
});
