// 时刻's 工夫: reading the start and length the reader typed, what a new entry starts from, and
// the history's daily curves. The server only checks structure; every rule here is local.
import { addDays, dayOf } from "./day";
import { parseClock } from "./sleep";

export type Activity = {
  id: string;
  type_id: string;
  type_name: string;
  started_at: number;
  minutes: number;
  note: string | null;
};
export type ActivityType = { id: string; name: string };

const MINUTE = 60_000;
const MAX_MINUTES = 12 * 60;

// Minutes as typed: "35", or "1:30" for the long ones.
export function parseLength(text: string): number | null {
  const trimmed = text.trim();
  if (/^\d+$/.test(trimmed)) return Number(trimmed);
  const match = /^(\d+)[:：](\d{2})$/.exec(trimmed);
  return match && Number(match[2]) < 60
    ? Number(match[1]) * 60 + Number(match[2])
    : null;
}

export type ActivityReading =
  | { kind: "activity"; started_at: number; minutes: number }
  | { kind: "invalid"; reason: string; field: "start" | "length" };

// An activity on `day` from the start and length typed. It must have ended by `now`.
export function readActivity(
  day: Date,
  start: string,
  length: string,
  now: Date,
): ActivityReading {
  const clock = parseClock(start);
  if (!clock)
    return {
      kind: "invalid",
      reason: "开始按 07:30 这样的格式填写",
      field: "start",
    };
  const minutes = parseLength(length);
  if (!minutes || minutes > MAX_MINUTES)
    return {
      kind: "invalid",
      reason: "时长填分钟数，或 1:30 这样的格式，不超过 12 小时",
      field: "length",
    };
  const startedAt = new Date(day);
  startedAt.setHours(clock[0], clock[1]);
  if (startedAt.getTime() + minutes * MINUTE > now.getTime())
    return { kind: "invalid", reason: "还没结束，结束后再记", field: "length" };
  return { kind: "activity", started_at: startedAt.getTime(), minutes };
}

// How long a type usually takes: the last ten of it, to the nearest five minutes; 30 if none.
export function usualMinutes(activities: Activity[], typeId: string): number {
  const recent = activities
    .filter((activity) => activity.type_id === typeId)
    .sort((a, b) => a.started_at - b.started_at)
    .slice(-10);
  if (recent.length === 0) return 30;
  const average =
    recent.reduce((sum, activity) => sum + activity.minutes, 0) / recent.length;
  return Math.max(5, Math.round(average / 5) * 5);
}

// Types with the most recently used first, so the usual one sits under the pointer; the rest
// keep the order they were added in.
export function typesByRecency(
  types: ActivityType[],
  activities: Activity[],
): ActivityType[] {
  const last = new Map<string, number>();
  for (const activity of activities)
    last.set(
      activity.type_id,
      Math.max(last.get(activity.type_id) ?? 0, activity.started_at),
    );
  return types
    .map((type, index) => ({ type, index, at: last.get(type.id) ?? -1 }))
    .sort((a, b) => b.at - a.at || a.index - b.index)
    .map((entry) => entry.type);
}

// Where a new entry starts: today, just finished; on another day, at seven in the morning.
// Rounded down to five minutes, the grain people remember times in.
export function defaultStart(day: Date, minutes: number, now: Date): Date {
  const start =
    dayOf(now.getTime()).getTime() === day.getTime()
      ? new Date(now.getTime() - minutes * MINUTE)
      : new Date(day.getFullYear(), day.getMonth(), day.getDate(), 7, 0);
  start.setMinutes(Math.floor(start.getMinutes() / 5) * 5, 0, 0);
  return start;
}

// Each type's minutes per day over `days` local days ending with `last`, oldest first. A day
// without that type is 0. Types are ordered by their total, largest first.
export function dailySeries(
  activities: Activity[],
  last: Date,
  days: number,
): { type: ActivityType; minutes: number[]; times: number }[] {
  const first = addDays(last, -(days - 1));
  const series = new Map<
    string,
    { type: ActivityType; minutes: number[]; times: number }
  >();
  for (const activity of activities) {
    const day = dayOf(activity.started_at);
    const index = Math.round((day.getTime() - first.getTime()) / 86_400_000);
    if (index < 0 || index >= days) continue;
    let entry = series.get(activity.type_id);
    if (!entry) {
      entry = {
        type: { id: activity.type_id, name: activity.type_name },
        minutes: Array<number>(days).fill(0),
        times: 0,
      };
      series.set(activity.type_id, entry);
    }
    entry.minutes[index] += activity.minutes;
    entry.times++;
  }
  const total = (list: number[]) => list.reduce((sum, value) => sum + value, 0);
  return [...series.values()].sort(
    (a, b) => total(b.minutes) - total(a.minutes),
  );
}

// A rounded line through every point, as d3's curveMonotoneX draws it. Monotone cubic
// interpolation never overshoots between two points, so a dip to an empty day stays on the
// baseline and no curve rises above the labelled peak.
export function smoothPath(points: [number, number][]): string {
  const n = points.length;
  if (n === 0) return "";
  const slope = (a: [number, number], b: [number, number]) =>
    (b[1] - a[1]) / (b[0] - a[0]);
  const secants = points.slice(1).map((point, i) => slope(points[i], point));
  // A turning point gets a flat tangent; elsewhere the tangent is capped to stay monotone.
  const tangents = points.map((_, i) => {
    if (n === 1) return 0;
    if (i === 0) return secants[0];
    if (i === n - 1) return secants[n - 2];
    const [before, after] = [secants[i - 1], secants[i]];
    if (before * after <= 0) return 0;
    return (
      Math.sign(before) *
      Math.min(
        2 * Math.abs(before),
        2 * Math.abs(after),
        Math.abs(before + after) / 2,
      )
    );
  });
  const f = (value: number) => Number(value.toFixed(1));
  let path = `M${f(points[0][0])},${f(points[0][1])}`;
  for (let i = 1; i < n; i++) {
    const [x0, y0] = points[i - 1];
    const [x1, y1] = points[i];
    const third = (x1 - x0) / 3;
    path += ` C${f(x0 + third)},${f(y0 + tangents[i - 1] * third)} ${f(x1 - third)},${f(y1 - tangents[i] * third)} ${f(x1)},${f(y1)}`;
  }
  return path;
}

// Minutes as the history writes them, e.g. "1:05".
export function hoursMinutes(minutes: number): string {
  return `${Math.floor(minutes / 60)}:${String(minutes % 60).padStart(2, "0")}`;
}
