import { useLayoutEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Link, useSearchParams } from "react-router-dom";
import { activitiesOptions, habitBoardOptions } from "@/lib/api";
import {
  addDays,
  backLabel,
  dayParam,
  dayRange,
  parseDay,
  today,
} from "@/lib/day";
import {
  checkKey,
  countsOf,
  fillOf,
  gridDays,
  nextCount,
  recentRate,
  type Habit,
} from "@/lib/habits";
import { dailySeries, hoursMinutes, smoothPath } from "@/lib/activities";
import { ErrorState } from "@/components/error-state";
import { fillStyle, useHabitTap } from "@/components/habit-seals";

const WEEKDAYS = ["日", "一", "二", "三", "四", "五", "六"];
const CURVE_DAYS = 30;

/**
 * 日课 and 工夫 over the last weeks: every habit as a row of nine weeks of days, each cell ticked
 * like its seal, and every type of activity as a curve of its daily minutes over 30 days.
 */
export function HabitsPage() {
  const [params] = useSearchParams();
  // The day the reader came from; the back link returns there.
  const from = parseDay(params.get("date"));
  return (
    <>
      <header className="sleep-head">
        {from && (
          <Link className="back" to={`/?date=${dayParam(from)}`}>
            {backLabel(from)}
          </Link>
        )}
        <h1>日课</h1>
      </header>
      <HabitGrid />
      <h2 className="habits-subhead">工夫</h2>
      <ActivityCurves />
    </>
  );
}

// Each habit a row, each day a cell filled as its seal would be; a tap ticks that day.
function HabitGrid() {
  const last = today();
  const days = gridDays(last);
  const options = habitBoardOptions(dayParam(days[0]), dayParam(last));
  const board = useQuery(options);
  const tap = useHabitTap();
  if (board.isError)
    return (
      <ErrorState error={board.error} retry={() => void board.refetch()} />
    );
  if (!board.data) return null;
  if (board.data.habits.length === 0)
    return <p className="sleep-empty">还没有日课。</p>;
  const counts = countsOf(board.data.checks);
  // A month is named where it begins, and at the start unless it begins within the first days.
  const months = days.flatMap((day, index) =>
    (index === 0 && !days.slice(1, 10).some((next) => next.getDate() === 1)) ||
    (index > 0 && day.getDate() === 1)
      ? [{ index, label: `${day.getMonth() + 1} 月` }]
      : [],
  );
  const offset = (index: number) =>
    index * 8 +
    days.slice(1, index + 1).filter((day) => day.getDay() === 1).length * 4;

  return (
    <div className="habit-grid-wrap">
      <div className="habit-grid">
        <span />
        <div className="habit-months" aria-hidden="true">
          {months.map((month) => (
            <span key={month.index} style={{ left: offset(month.index) }}>
              {month.label}
            </span>
          ))}
        </div>
        <span className="rate">近 30 天</span>
        {board.data.habits.map((habit) => (
          <HabitRow
            key={habit.id}
            habit={habit}
            days={days}
            counts={counts}
            onTap={(day, count) =>
              tap.mutate({
                habit,
                day,
                count: nextCount(count, habit.times_per_day),
                queryKey: options.queryKey,
              })
            }
          />
        ))}
      </div>
    </div>
  );
}

function HabitRow({
  habit,
  days,
  counts,
  onTap,
}: {
  habit: Habit;
  days: Date[];
  counts: Map<string, number>;
  onTap: (day: string, count: number) => void;
}) {
  const last = days[days.length - 1];
  return (
    <>
      <span className="name" data-retired={habit.retired} title={habit.name}>
        {habit.name}
      </span>
      <div className="cells">
        {days.map((day, index) => {
          const date = dayParam(day);
          const count = counts.get(checkKey(habit.id, date)) ?? 0;
          const fill = fillOf(count, habit.times_per_day);
          const shown = Math.min(count, habit.times_per_day);
          return (
            <button
              key={date}
              type="button"
              className="cell"
              data-fill={fill.kind}
              data-week={index > 0 && day.getDay() === 1 ? "start" : undefined}
              data-today={date === dayParam(last) || undefined}
              style={fillStyle(fill)}
              // A retired habit takes no checks; its days are only shown.
              disabled={habit.retired}
              aria-label={`${day.getMonth() + 1}/${day.getDate()} 周${WEEKDAYS[day.getDay()]} ${habit.name} ${shown}/${habit.times_per_day}`}
              onClick={() => onTap(date, count)}
            />
          );
        })}
      </div>
      <span className="rate">
        {Math.round(recentRate(habit, counts, last) * 100)}%
      </span>
    </>
  );
}

const LABEL_WIDTH = 132;
const CHART_HEIGHT = 60;
const PEAK_ROOM = 18;

// Every type of activity as a curve of its minutes per day over the last 30 days, one row each,
// on a shared day axis with one crosshair across all rows.
function ActivityCurves() {
  const last = today();
  const first = addDays(last, -(CURVE_DAYS - 1));
  const activities = useQuery(
    activitiesOptions(dayParam(first), dayParam(last), {
      start: dayRange(first).start,
      end: dayRange(last).end,
    }),
  );
  const box = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(520);
  const [hover, setHover] = useState<{ index: number; type: string } | null>(
    null,
  );
  useLayoutEffect(() => {
    // The rows share the sheet's width less the labels; jsdom measures nothing, so keep a default.
    function measure() {
      const room = (box.current?.clientWidth ?? 0) - LABEL_WIDTH - 16;
      if (room > 0) setWidth(Math.max(240, room));
    }
    measure();
    window.addEventListener("resize", measure);
    return () => window.removeEventListener("resize", measure);
  }, [activities.data]);

  if (activities.isError)
    return (
      <ErrorState
        error={activities.error}
        retry={() => void activities.refetch()}
      />
    );
  if (!activities.data) return null;
  const series = dailySeries(activities.data, last, CURVE_DAYS);
  if (series.length === 0)
    return <p className="sleep-empty">近 30 天没有工夫。</p>;

  const days = [...Array(CURVE_DAYS).keys()].map((back) =>
    addDays(first, back),
  );
  const x = (index: number) => 4 + (index * (width - 8)) / (CURVE_DAYS - 1);
  const dayAt = (clientX: number, left: number) =>
    Math.max(
      0,
      Math.min(
        CURVE_DAYS - 1,
        Math.round(((clientX - left - 4) / (width - 8)) * (CURVE_DAYS - 1)),
      ),
    );

  return (
    <div className="activity-curves" ref={box}>
      {series.map(({ type, minutes, times }) => {
        // Each row scales to its own peak: the point is how one activity swings.
        const peak = Math.max(...minutes);
        const peakIndex = minutes.indexOf(peak);
        const y = (value: number) =>
          PEAK_ROOM + (CHART_HEIGHT - PEAK_ROOM - 1) * (1 - value / peak);
        const curve = smoothPath(
          minutes.map((value, index) => [x(index), y(value)]),
        );
        const total = minutes.reduce((sum, value) => sum + value, 0);
        const anchor =
          peakIndex < 3
            ? "start"
            : peakIndex > CURVE_DAYS - 4
              ? "end"
              : "middle";
        return (
          <div className="activity-row" key={type.id}>
            <div className="activity-label">
              <b>{type.name}</b>
              <small>
                {times} 次 · 共 {hoursMinutes(total)}
              </small>
            </div>
            <div className="activity-chart">
              <svg
                width={width}
                height={CHART_HEIGHT}
                viewBox={`0 0 ${width} ${CHART_HEIGHT}`}
                role="img"
                aria-label={`${type.name}近 30 天每天的时长，最多 ${hoursMinutes(peak)}`}
                onMouseMove={(event) =>
                  setHover({
                    index: dayAt(
                      event.clientX,
                      event.currentTarget.getBoundingClientRect().left,
                    ),
                    type: type.id,
                  })
                }
                onMouseLeave={() => setHover(null)}
              >
                <line
                  className="base"
                  x1={0}
                  x2={width}
                  y1={CHART_HEIGHT - 0.5}
                  y2={CHART_HEIGHT - 0.5}
                />
                <path
                  className="wash"
                  d={`${curve} L${x(CURVE_DAYS - 1)},${CHART_HEIGHT} L${x(0)},${CHART_HEIGHT} Z`}
                />
                <path className="trace" d={curve} />
                <circle cx={x(peakIndex)} cy={y(peak)} r={4} />
                <text
                  className="peak"
                  x={x(peakIndex)}
                  y={y(peak) - 9}
                  textAnchor={anchor}
                >
                  {hoursMinutes(peak)}
                </text>
                {hover && (
                  <g className="cross">
                    <line
                      x1={x(hover.index)}
                      x2={x(hover.index)}
                      y1={2}
                      y2={CHART_HEIGHT}
                    />
                    <circle
                      cx={x(hover.index)}
                      cy={y(minutes[hover.index])}
                      r={4}
                    />
                  </g>
                )}
              </svg>
              {hover?.type === type.id && (
                <div className="activity-tip" style={{ left: x(hover.index) }}>
                  {`${days[hover.index].getMonth() + 1}/${days[hover.index].getDate()} 周${WEEKDAYS[days[hover.index].getDay()]} · `}
                  {minutes[hover.index]
                    ? `${type.name} ${minutes[hover.index]} 分钟`
                    : `没有${type.name}`}
                </div>
              )}
            </div>
          </div>
        );
      })}
      <div className="activity-row">
        <span />
        <div className="activity-axis" style={{ width }}>
          {days.map((day, index) =>
            index === CURVE_DAYS - 1 ? (
              <span key={index} className="end" style={{ left: x(index) }}>
                今天
              </span>
            ) : day.getDay() === 1 && index < CURVE_DAYS - 3 ? (
              <span key={index} style={{ left: x(index) }}>
                {day.getMonth() + 1}/{day.getDate()}
              </span>
            ) : null,
          )}
        </div>
      </div>
    </div>
  );
}
