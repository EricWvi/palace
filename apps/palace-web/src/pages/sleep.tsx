import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Link, useSearchParams } from "react-router-dom";
import { sleepNightsOptions } from "@/lib/api";
import {
  addDays,
  backLabel,
  dayOf,
  dayParam,
  dayRange,
  parseDay,
  today,
} from "@/lib/day";
import {
  averageClock,
  averageDuration,
  axisPercent,
  clock,
  longDuration,
  minutesBetween,
  nightOn,
  shortDuration,
} from "@/lib/sleep";
import { ErrorState } from "@/components/error-state";
import {
  SleepNightDialog,
  type NightTarget,
} from "@/components/sleep-night-dialog";

// A year of local days, newest first; older nights stay stored but are not listed.
const DAYS = 365;
const WEEKDAYS = ["日", "一", "二", "三", "四", "五", "六"];
// Hour marks under the axis, as hours after its 20:00 start.
const TICKS = [22, 0, 2, 4, 6, 8];

/**
 * Every night of the last year, one row a day on the same evening-to-morning axis, so drift in
 * bedtime shows as a slant. A recorded row opens for correction, an empty one to be filled in.
 */
export function SleepPage() {
  const [params] = useSearchParams();
  // The day the reader came from; the back link returns there.
  const from = parseDay(params.get("date"));
  const last = today();
  const first = addDays(last, -(DAYS - 1));
  const nights = useQuery(
    sleepNightsOptions(dayParam(first), dayParam(last), {
      start: dayRange(first).start,
      end: dayRange(last).end,
    }),
  );
  const [target, setTarget] = useState<NightTarget | null>(null);

  const recent = (nights.data ?? []).slice(-30);
  const days = [...Array(DAYS).keys()].map((back) => addDays(last, -back));
  const oldest = nights.data?.[0];
  // Rows start at the first listed night, so a new history is not a year of blanks.
  const shown = oldest
    ? days.filter((day) => day >= dayOf(oldest.ended_at))
    : [];

  return (
    <>
      <header className="sleep-head">
        {from && (
          <Link className="back" to={`/?date=${dayParam(from)}`}>
            {backLabel(from)}
          </Link>
        )}
        <h1>睡眠</h1>
      </header>
      {nights.isError ? (
        <ErrorState error={nights.error} retry={() => void nights.refetch()} />
      ) : nights.data === undefined ? null : nights.data.length === 0 ? (
        <p className="sleep-empty">还没有睡眠记录。</p>
      ) : (
        <>
          <div className="sleep-summary">
            <div>
              <small>近 30 晚平均</small>
              <b>{longDuration(averageDuration(recent) ?? 0)}</b>
            </div>
            <div>
              <small>平均入睡</small>
              <b>{averageClock(recent, "started_at")}</b>
            </div>
            <div>
              <small>平均醒来</small>
              <b>{averageClock(recent, "ended_at")}</b>
            </div>
          </div>
          <ol className="nights">
            {shown.map((day) => {
              const night = nightOn(nights.data, day);
              const selected =
                target !== null && target.day.getTime() === day.getTime();
              return (
                <li
                  key={day.getTime()}
                  // A week starts on Monday; the hairline sits above the Sunday before it.
                  className={
                    day.getDay() === 0 && day.getTime() !== last.getTime()
                      ? "week-start"
                      : undefined
                  }
                  aria-current={selected || undefined}
                >
                  <button
                    type="button"
                    title={
                      night
                        ? `${clock(night.started_at)} – ${clock(night.ended_at)}`
                        : "没有记录"
                    }
                    onClick={() =>
                      setTarget(
                        night
                          ? { kind: "edit", day, night }
                          : { kind: "fill", day },
                      )
                    }
                  >
                    <span className="when">
                      {day.getMonth() + 1}/{day.getDate()}
                      <span className="weekday">
                        {" "}
                        周{WEEKDAYS[day.getDay()]}
                      </span>
                    </span>
                    <span className="axis">
                      {night && (
                        <span
                          className="bar"
                          style={{
                            left: `${axisPercent(day, night.started_at)}%`,
                            right: `${100 - axisPercent(day, night.ended_at)}%`,
                          }}
                        />
                      )}
                    </span>
                    <span className="length">
                      {night ? (
                        shortDuration(
                          minutesBetween(night.started_at, night.ended_at),
                        )
                      ) : (
                        <span className="missing">无</span>
                      )}
                    </span>
                  </button>
                </li>
              );
            })}
          </ol>
          <div className="ticks" aria-hidden="true">
            {TICKS.map((hour, index) => (
              <span
                key={hour}
                style={{ left: `${((index * 2 + 2) / 14) * 100}%` }}
              >
                {String(hour).padStart(2, "0")}:00
              </span>
            ))}
          </div>
        </>
      )}
      {target && (
        <SleepNightDialog
          key={`${target.kind}-${target.day.getTime()}`}
          target={target}
          usual={[
            averageClock(recent, "started_at") ?? "23:30",
            averageClock(recent, "ended_at") ?? "07:00",
          ]}
          onClose={() => setTarget(null)}
        />
      )}
    </>
  );
}
