// 时刻's sleep rules on the reader's clock. The server only keeps nights from overlapping and
// lasting over a day; every hour named here is local, decided in the browser.
import { addDays, dayOf } from "./day";

export type Night = { id: string; started_at: number; ended_at: number };

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;

// What the day's menu offers for sleep right now, if anything. A running sleep ends from its
// own dial, so the menu never offers ending.
export function sleepAction(
  now: Date,
  state: { running: boolean; sleptToday: boolean },
): "start" | "record" | null {
  if (state.running) return null;
  const hour = now.getHours();
  if (hour >= 18 || hour < 5) return "start";
  if (hour < 15 && !state.sleptToday) return "record";
  return null;
}

// A start still running at 15:00 of the day after its evening was forgotten: it no longer shows,
// and the next start replaces it on the server.
export function expired(startedAt: number, now: Date): boolean {
  const start = new Date(startedAt);
  const evening = dayOf(startedAt);
  const eveningOf = start.getHours() >= 12 ? evening : addDays(evening, -1);
  const deadline = addDays(eveningOf, 1);
  deadline.setHours(15);
  return now.getTime() >= deadline.getTime();
}

// Before 06:00, counting the evening before as well, a tap on the stop button is more likely a
// slip than a waking, so it asks once more.
export function asksBeforeEnding(now: Date): boolean {
  return now.getHours() < 6 || now.getHours() >= 18;
}

// Reads "23:30", "2330" or "23：30" as hours and minutes.
export function parseClock(text: string): [number, number] | null {
  const match = /^(\d{1,2})[:：.]?(\d{2})$/.exec(text.trim());
  if (!match) return null;
  const [hours, minutes] = [Number(match[1]), Number(match[2])];
  return hours < 24 && minutes < 60 ? [hours, minutes] : null;
}

export type NightReading =
  | { kind: "night"; started_at: number; ended_at: number }
  | { kind: "invalid"; reason: string; field: "bed" | "wake" };

// The night filed under `day` from two typed clock times. A bedtime from noon on is the evening
// before; an earlier one is the small hours of the day itself. Waking is on the day.
export function readNight(
  day: Date,
  bed: string,
  wake: string,
  now: Date,
): NightReading {
  const bedtime = parseClock(bed);
  const waking = parseClock(wake);
  if (!bedtime || !waking)
    return {
      kind: "invalid",
      reason: "按 23:30 这样的格式填写",
      field: bedtime ? "wake" : "bed",
    };
  const startDay = bedtime[0] >= 12 ? addDays(day, -1) : day;
  const start = new Date(startDay);
  start.setHours(...bedtime);
  const end = new Date(day);
  end.setHours(...waking);
  if (end <= start)
    return { kind: "invalid", reason: "醒来要晚于入睡", field: "wake" };
  if (end > now)
    return { kind: "invalid", reason: "醒来时间还没到", field: "wake" };
  if (end.getTime() - start.getTime() > 16 * HOUR)
    return {
      kind: "invalid",
      reason: "超过 16 小时，检查一下入睡时间",
      field: "bed",
    };
  return {
    kind: "night",
    started_at: start.getTime(),
    ended_at: end.getTime(),
  };
}

const pad = (value: number) => String(value).padStart(2, "0");

export function clock(time: number): string {
  const date = new Date(time);
  return `${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

export function minutesBetween(from: number, to: number): number {
  return Math.round((to - from) / MINUTE);
}

// "7 小时 26 分".
export function longDuration(minutes: number): string {
  return `${Math.floor(minutes / 60)} 小时 ${pad(minutes % 60)} 分`;
}

// "7:26".
export function shortDuration(minutes: number): string {
  return `${Math.floor(minutes / 60)}:${pad(minutes % 60)}`;
}

// "1:23:04", for the running dial.
export function stopwatch(ms: number): string {
  const seconds = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(seconds / 3600)}:${pad(Math.floor(seconds / 60) % 60)}:${pad(seconds % 60)}`;
}

// The night filed under `day`: the latest waking, if a day ever holds more than one.
export function nightOn(nights: Night[], day: Date): Night | undefined {
  const time = day.getTime();
  return nights
    .filter((night) => dayOf(night.ended_at).getTime() === time)
    .at(-1);
}

// Average clock time of the nights' bedtimes or wakings. Bedtimes straddle midnight, so they
// average as minutes after 18:00 of the evening before; wakings as minutes after midnight.
export function averageClock(
  nights: Night[],
  pick: "started_at" | "ended_at",
): string | null {
  if (nights.length === 0) return null;
  const base = pick === "started_at" ? 18 : 0;
  const offsets = nights.map((night) => {
    const origin = dayOf(night.ended_at);
    if (pick === "started_at") origin.setDate(origin.getDate() - 1);
    origin.setHours(base);
    return minutesBetween(origin.getTime(), night[pick]);
  });
  const average = Math.round(
    offsets.reduce((sum, value) => sum + value, 0) / offsets.length,
  );
  const minutes = (((base * 60 + average) % 1440) + 1440) % 1440;
  return `${pad(Math.floor(minutes / 60))}:${pad(minutes % 60)}`;
}

export function averageDuration(nights: Night[]): number | null {
  if (nights.length === 0) return null;
  const total = nights.reduce(
    (sum, night) => sum + minutesBetween(night.started_at, night.ended_at),
    0,
  );
  return Math.round(total / nights.length);
}

// Where a moment sits on a night's axis, 20:00 of the evening before the wake day to 10:00 of it,
// as a percentage clamped to the axis.
export function axisPercent(wakeDay: Date, time: number): number {
  const origin = addDays(wakeDay, -1);
  origin.setHours(20);
  const percent = ((time - origin.getTime()) / (14 * HOUR)) * 100;
  return Math.min(100, Math.max(0, percent));
}
