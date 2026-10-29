import { useCallback, useLayoutEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import {
  ApiError,
  api,
  apiData,
  refreshSleep,
  sleepNightsOptions,
  sleepRunningOptions,
} from "@/lib/api";
import { addDays, dayParam, dayRange } from "@/lib/day";
import {
  asksBeforeEnding,
  clock,
  expired,
  longDuration,
  minutesBetween,
  nightOn,
  stopwatch,
  type Night,
} from "@/lib/sleep";
import { ErrorState } from "./error-state";
import { Button } from "./ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "./ui/dialog";

// The seven local days ending with `day`, as one range of wake times.
export function weekOf(day: Date) {
  const first = addDays(day, -6);
  return {
    from: dayParam(first),
    to: dayParam(day),
    range: { start: dayRange(first).start, end: dayRange(day).end },
  };
}

/**
 * The sleep dial pinned under the day's heading: today's running sleep while it runs, otherwise
 * the night filed under the day, otherwise nothing. Either leads to the sleep history.
 * `expectsNight` says the day's outline or cards hold a sleep moment, so an empty dial can hold
 * its place until the night arrives instead of the cards below jumping down.
 */
export function SleepPin({
  day,
  isToday,
  expectsNight,
}: {
  day: Date;
  isToday: boolean;
  expectsNight: boolean;
}) {
  const week = weekOf(day);
  const nights = useQuery(sleepNightsOptions(week.from, week.to, week.range));
  const running = useQuery({ ...sleepRunningOptions(), enabled: isToday });
  // Bumped when a running start lapses on screen, so the dial gives way without a refetch.
  const [, setLapsed] = useState(0);
  const lapse = useCallback(() => setLapsed((count) => count + 1), []);
  const history = `/sleep?date=${dayParam(day)}`;

  const startedAt = isToday ? running.data?.started_at : null;
  if (startedAt != null && !expired(startedAt, new Date()))
    return (
      <RunningDial startedAt={startedAt} history={history} onLapse={lapse} />
    );
  if (!nights.data)
    return expectsNight ? (
      <div className="dial placeholder" aria-hidden="true" />
    ) : null;
  const night = nightOn(nights.data, day);
  if (!night) return null;
  return (
    <NightDial night={night} nights={nights.data} day={day} history={history} />
  );
}

// A finished night: its length large, bedtime and waking beside it, and the week's nights as
// bars, each a fraction of the longest; a day without a night is a short stroke.
function NightDial({
  night,
  nights,
  day,
  history,
}: {
  night: Night;
  nights: Night[];
  day: Date;
  history: string;
}) {
  const length = minutesBetween(night.started_at, night.ended_at);
  const week = [...Array(7).keys()].map((index) =>
    nightOn(nights, addDays(day, index - 6)),
  );
  const longest = Math.max(
    ...week.map((entry) =>
      entry ? minutesBetween(entry.started_at, entry.ended_at) : 0,
    ),
  );
  return (
    <Link
      className="dial"
      to={history}
      aria-label={`睡眠 ${longDuration(length)}，${clock(night.started_at)} 入睡，${clock(night.ended_at)} 醒来`}
    >
      <span className="big" aria-hidden="true">
        {Math.floor(length / 60)}
        <small>小时</small>
        {String(length % 60).padStart(2, "0")}
        <small>分</small>
      </span>
      <dl aria-hidden="true">
        <dt>入睡</dt>
        <dd>{clock(night.started_at)}</dd>
        <dt>醒来</dt>
        <dd>{clock(night.ended_at)}</dd>
      </dl>
      <span className="week" aria-hidden="true">
        {week.map((entry, index) =>
          entry ? (
            <i
              key={index}
              className={entry.id === night.id ? "this" : undefined}
              style={{
                height: `${(minutesBetween(entry.started_at, entry.ended_at) / longest) * 100}%`,
              }}
            />
          ) : (
            <i key={index} className="gap" />
          ),
        )}
      </span>
    </Link>
  );
}

// The running sleep: a stopwatch and a stop button. Only the figures change each second; the
// dial itself stays, so the button's breathing never restarts with the seconds.
function RunningDial({
  startedAt,
  history,
  onLapse,
}: {
  startedAt: number;
  history: string;
  onLapse: () => void;
}) {
  const figures = useRef<HTMLSpanElement>(null);
  // When the stop button asked, so the question names that moment; null while not asking.
  const [asking, setAsking] = useState<number | null>(null);
  const client = useQueryClient();
  const end = useMutation({
    mutationFn: () =>
      apiData(
        api.POST("/api/sleep/running/end", { body: { started_at: startedAt } }),
      ),
    onSuccess: async () => {
      setAsking(null);
      client.setQueryData(sleepRunningOptions().queryKey, { started_at: null });
      await refreshSleep(client);
    },
    onError: async (error) => {
      // Another device started again or ended it; show what is running now.
      if (error instanceof ApiError && error.status === 404)
        await client.invalidateQueries({ queryKey: ["sleep", "running"] });
    },
  });

  // The figures are written here and nowhere else, before the first paint and then each second;
  // React leaves the element's text alone.
  useLayoutEffect(() => {
    const tick = () => {
      const now = new Date();
      if (expired(startedAt, now)) return onLapse();
      if (figures.current)
        figures.current.textContent = stopwatch(now.getTime() - startedAt);
    };
    tick();
    const timer = setInterval(tick, 1000);
    return () => clearInterval(timer);
  }, [startedAt, onLapse]);

  const stop = () => {
    const now = new Date();
    if (asksBeforeEnding(now)) setAsking(now.getTime());
    else end.mutate();
  };
  return (
    <div className="dial running">
      {/* The whole dial leads to the history; the stop button sits above that link. */}
      <Link className="dial-link" to={history} aria-label="睡眠记录" />
      <span className="big" ref={figures} aria-label="已睡" />
      <dl>
        <dt>入睡</dt>
        <dd>{clock(startedAt)}</dd>
      </dl>
      <button
        type="button"
        className="end"
        aria-label="结束睡眠"
        title="结束睡眠"
        disabled={end.isPending}
        onClick={stop}
      />
      {end.isError && asking === null && <ErrorState error={end.error} />}
      <Dialog
        open={asking !== null}
        onOpenChange={(open) => {
          if (!open && !end.isPending) setAsking(null);
        }}
      >
        <DialogContent role="alertdialog">
          <DialogHeader>
            <DialogTitle>现在就结束睡眠？</DialogTitle>
            <DialogDescription>
              {asking !== null &&
                `现在 ${clock(asking)}，才睡了 ${longDuration(minutesBetween(startedAt, asking))}。`}
            </DialogDescription>
          </DialogHeader>
          {end.isError && <ErrorState error={end.error} />}
          <DialogFooter>
            <Button
              type="button"
              variant="ghost"
              disabled={end.isPending}
              onClick={() => end.mutate()}
            >
              {end.isPending ? "正在结束…" : "结束"}
            </Button>
            {/* Staying asleep is the safe answer, so it takes focus and Enter. */}
            <Button type="button" autoFocus onClick={() => setAsking(null)}>
              继续睡
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
