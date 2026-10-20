import type { JottingOverview } from "@/lib/api";
import { heatmapDays, localDay } from "@/lib/jotting-time";

// The sidebar keeps to the most used tags so it fits one screen however many pile up; the rest
// live in the all-tags view.
const SIDE_TAGS = 8;

// The left column under the recall pile: how much has been written, when, and the tags most
// used. A filter on a tag outside the top eight stays listed, so its state is never hidden.
export function EchoSide({
  overview,
  activeTag,
  allTags,
  onTag,
  onAllTags,
}: {
  overview: JottingOverview | undefined;
  activeTag: string;
  // The all-tags view is showing.
  allTags: boolean;
  onTag: (tag: string) => void;
  onAllTags: () => void;
}) {
  if (!overview) return null;
  const shown = overview.tags.slice(0, SIDE_TAGS);
  const active = overview.tags.find((tag) => tag.name === activeTag);
  if (active && !shown.includes(active)) shown.push(active);
  return (
    <div className="side-lists">
      <div className="stats">
        <div>
          <strong>{overview.total}</strong>
          <span>札记</span>
        </div>
        <div>
          <strong>{overview.tags.length}</strong>
          <span>标签</span>
        </div>
        <div>
          <strong>{overview.days}</strong>
          <span>天</span>
        </div>
      </div>
      <Heatmap daily={overview.daily} />
      <div className="side-list">
        {shown.map((tag) => (
          <button
            key={tag.name}
            type="button"
            aria-current={tag.name === activeTag ? "true" : undefined}
            onClick={() => onTag(tag.name)}
          >
            #{tag.name}
            <small>{tag.count}</small>
          </button>
        ))}
        <button
          type="button"
          className="side-all"
          aria-current={allTags ? "true" : undefined}
          onClick={onAllTags}
        >
          全部标签<small>{overview.tags.length}</small>
        </button>
      </div>
    </div>
  );
}

// Twelve weeks of jottings per day, Monday on top and this week on the right, in four shades.
// Only to look at: a day is not a filter.
function Heatmap({ daily }: { daily: JottingOverview["daily"] }) {
  const counts = new Map(daily.map((day) => [day.date, day.count]));
  const today = localDay(new Date());
  return (
    <div className="heat" aria-hidden="true">
      {heatmapDays(new Date()).map((day) => {
        if (day > today) return <i key={day} data-future="" />;
        const count = counts.get(day) ?? 0;
        return (
          <i
            key={day}
            data-level={Math.min(count, 3)}
            title={`${day} · ${count} 张`}
          />
        );
      })}
    </div>
  );
}

// Every tag as one flowing paragraph, for the sidebar's long tail. `filter` narrows it live.
export function TagFlow({
  overview,
  activeTag,
  filter,
  onTag,
  onBack,
}: {
  overview: JottingOverview | undefined;
  activeTag: string;
  filter: string;
  onTag: (tag: string) => void;
  onBack: () => void;
}) {
  const all = overview?.tags ?? [];
  const needle = filter.trim().toLowerCase();
  const rows = all.filter((tag) => tag.name.toLowerCase().includes(needle));
  return (
    <section className="tag-view" aria-label="全部标签">
      <p className="filter-line">
        <strong>全部标签</strong>
        <span>{needle ? `${rows.length} / ${all.length}` : all.length} 个</span>
        <button type="button" onClick={onBack}>
          返回
        </button>
      </p>
      <div className="tag-flow">
        {rows.length === 0 ? (
          <p className="empty">没有找到。</p>
        ) : (
          rows.map((tag) => (
            <button
              key={tag.name}
              type="button"
              aria-current={tag.name === activeTag ? "true" : undefined}
              onClick={() => onTag(tag.name)}
            >
              #{tag.name}
              <small>({tag.count})</small>
            </button>
          ))
        )}
      </div>
    </section>
  );
}
