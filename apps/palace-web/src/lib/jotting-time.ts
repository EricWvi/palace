const MINUTE = 60_000;
const pad = (n: number) => String(n).padStart(2, "0");

// `YYYY-MM-DD` of a local calendar day, the shape the overview's `daily` uses.
export function localDay(date: Date): string {
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

// `YYYY-MM-DD HH:mm` in the reader's zone.
export function absoluteTime(time: number): string {
  const date = new Date(time);
  return `${localDay(date)} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

// Relative wording only inside seven days; null past that, where the full time shows instead.
export function relativeTime(time: number, now: number): string | null {
  const minutes = Math.max(0, (now - time) / MINUTE);
  if (minutes < 1) return "刚刚";
  if (minutes < 60) return `${Math.floor(minutes)} 分钟前`;
  if (minutes < 24 * 60) return `${Math.floor(minutes / 60)} 小时前`;
  if (minutes < 7 * 24 * 60) return `${Math.floor(minutes / 60 / 24)} 天前`;
  return null;
}

// The heatmap's twelve weeks: every local day from the Monday eleven weeks before this week's
// Monday through this week's Sunday, so each column is one week with Monday on top.
export function heatmapDays(today: Date): string[] {
  const monday = new Date(
    today.getFullYear(),
    today.getMonth(),
    today.getDate() - ((today.getDay() + 6) % 7) - 77,
  );
  return Array.from({ length: 84 }, (_, i) =>
    localDay(
      new Date(monday.getFullYear(), monday.getMonth(), monday.getDate() + i),
    ),
  );
}
