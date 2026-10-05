// A day is always the browser's local calendar day; the server only ever sees its bounds.

// Formats a local day as the `date` query value, e.g. 2025-09-30.
export function dayParam(day: Date): string {
  const pad = (value: number, width = 2) => String(value).padStart(width, "0");
  return `${pad(day.getFullYear(), 4)}-${pad(day.getMonth() + 1)}-${pad(day.getDate())}`;
}

// Reads a `date` query value; anything unparsable or out of the calendar means "no day".
export function parseDay(value: string | null): Date | null {
  const match = value?.match(/^(\d{4})-(\d{2})-(\d{2})$/);
  if (!match) return null;
  const [year, month, date] = match.slice(1).map(Number);
  const day = new Date(year, month - 1, date);
  // Date rolls 2025-02-30 over into March; a rolled day was never a real date.
  if (day.getMonth() !== month - 1 || day.getDate() !== date) return null;
  day.setFullYear(year);
  return day;
}

export function today(): Date {
  const now = new Date();
  return new Date(now.getFullYear(), now.getMonth(), now.getDate());
}

export function addDays(day: Date, amount: number): Date {
  return new Date(day.getFullYear(), day.getMonth(), day.getDate() + amount);
}

// The half-open epoch range of a local day; it spans 23 or 25 hours across DST changes.
export function dayRange(day: Date): { start: number; end: number } {
  return { start: day.getTime(), end: addDays(day, 1).getTime() };
}

// The local day an epoch instant falls on.
export function dayOf(time: number): Date {
  const instant = new Date(time);
  return new Date(instant.getFullYear(), instant.getMonth(), instant.getDate());
}

// The 时刻 heading, e.g. "Sep 30, 2025", as in the prototype.
export function dayHeading(day: Date): string {
  return new Intl.DateTimeFormat("en-US", {
    month: "short",
    day: "numeric",
    year: "numeric",
  }).format(day);
}

// The way back to a day, e.g. "← 9 月 30 日".
export function backLabel(day: Date): string {
  return `← ${day.getMonth() + 1} 月 ${day.getDate()} 日`;
}

// A new record defaults to the viewed day at the current local time of day.
export function atCurrentTime(day: Date): Date {
  const now = new Date();
  return new Date(
    day.getFullYear(),
    day.getMonth(),
    day.getDate(),
    now.getHours(),
    now.getMinutes(),
  );
}
