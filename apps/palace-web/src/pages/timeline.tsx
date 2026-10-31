import {
  Suspense,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate, useSearchParams } from "react-router-dom";
import {
  api,
  apiData,
  habitBoardOptions,
  outlineOptions,
  sleepNightsOptions,
  sleepRunningOptions,
  timelineOptions,
} from "@/lib/api";
import { averageClock, expired, nightOn, sleepAction } from "@/lib/sleep";
import { preloadable } from "@/lib/preloadable";
import {
  addDays,
  dayHeading,
  dayOf,
  dayParam,
  dayRange,
  parseDay,
  today,
} from "@/lib/day";
import {
  ActivityDialog,
  type ActivityTarget,
} from "@/components/activity-dialog";
import { DayMoments } from "@/components/day-moments";
import { HabitArrangeDialog } from "@/components/habit-arrange-dialog";
import { HabitSeals } from "@/components/habit-seals";
import { ErrorState } from "@/components/error-state";
import { MoreMenu, type MenuAction } from "@/components/more-menu";
import { SleepPin, weekOf } from "@/components/sleep-pin";
import {
  SleepNightDialog,
  type NightTarget,
} from "@/components/sleep-night-dialog";
import {
  useShortcutHelp,
  type ShortcutGroup,
} from "@/components/shortcut-help";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";

// The calendar is fetched on first approach to the date, and the import form brings it along;
// most visits never open either.
const calendar = preloadable(() =>
  import("@/components/day-calendar").then((module) => module.DayCalendar),
);
const DayCalendar = calendar.Component;
// The import dialog carries its own date picker and calendar, too much to fetch on the click
// that opens it: the "···" in front of it starts the fetch as soon as it is approached.
const importDialog = preloadable(() =>
  import("@/components/import-dialog").then((module) => module.ImportDialog),
);
const ImportDialog = importDialog.Component;
// Key management is opened rarely enough to load only when the "···" is approached, as above.
const keyManager = preloadable(() =>
  import("@/components/key-manager").then((module) => module.KeyManager),
);
const KeyManager = keyManager.Component;

const shortcuts: ShortcutGroup = {
  title: "时刻",
  shortcuts: [
    { keys: ["←"], action: "前一天" },
    { keys: ["→"], action: "后一天" },
    { keys: ["g"], action: "记录工夫" },
  ],
};

export function TimelinePage() {
  const [params, setParams] = useSearchParams();
  const navigate = useNavigate();
  // An unreadable date falls back to today rather than an error page.
  const day = parseDay(params.get("date")) ?? today();
  const date = dayParam(day);
  const range = dayRange(day);
  const query = useQuery(timelineOptions(date, range));
  // Asked alongside the cards, never before them, and only when the day has no cards to show.
  const outline = useQuery({
    ...outlineOptions(date, range),
    enabled: query.isPending,
  });
  const [importing, setImporting] = useState(false);
  const [managingKeys, setManagingKeys] = useState(false);
  const [picking, setPicking] = useState(false);
  const [recording, setRecording] = useState<NightTarget | null>(null);
  const [activity, setActivity] = useState<ActivityTarget | null>(null);
  const [arranging, setArranging] = useState(false);
  const habits = useQuery(habitBoardOptions(date, date));
  const isToday = date === dayParam(today());
  // The menu's sleep action follows the clock; it is read when the "···" is approached, so a page
  // left open from the afternoon into the evening still offers starting.
  const [menuTime, setMenuTime] = useState(() => new Date());
  const week = weekOf(day);
  const nights = useQuery(sleepNightsOptions(week.from, week.to, week.range));
  const running = useQuery({ ...sleepRunningOptions(), enabled: isToday });
  const client = useQueryClient();
  const start = useMutation({
    mutationFn: () => apiData(api.PUT("/api/sleep/running")),
    onSuccess: (started) =>
      client.setQueryData(sleepRunningOptions().queryKey, started),
  });
  // Sleep is pinned above the list, so the list and its placeholders leave it out.
  const cards = useMemo(
    () => query.data?.filter((moment) => moment.kind !== "sleep"),
    [query.data],
  );
  const shapes = useMemo(
    () => outline.data?.filter((entry) => entry.kind !== "sleep"),
    [outline.data],
  );
  const expectsNight = Boolean(
    (query.data ?? outline.data)?.some((entry) => entry.kind === "sleep"),
  );
  // How the calendar was opened decides where focus goes once it closes, see below.
  const opened = useRef<"pointer" | "keyboard">("keyboard");

  // Sleep is started and recorded only for today; a running sleep ends from its own dial. Until
  // both the running start and today's nights are known, nothing is offered.
  const startedAt = running.data?.started_at;
  const offered =
    isToday && running.data && nights.data
      ? sleepAction(menuTime, {
          running: startedAt != null && !expired(startedAt, menuTime),
          sleptToday: nightOn(nights.data, day) !== undefined,
        })
      : null;
  const sleepActions: MenuAction[] =
    offered === "start"
      ? [{ label: "开始睡眠", onSelect: () => start.mutate() }]
      : offered === "record"
        ? [
            {
              label: "记录睡眠",
              onSelect: () => setRecording({ kind: "record", day }),
            },
          ]
        : [];

  // Replace, never push: stepping through days is browsing, so back still leaves the timeline.
  function show(next: Date) {
    setParams({ date: dayParam(next) }, { replace: true });
  }

  useShortcutHelp(shortcuts);
  useEffect(() => {
    function step(event: KeyboardEvent) {
      if (event.metaKey || event.ctrlKey || event.altKey) return;
      const target = event.target as HTMLElement;
      if (target.closest("input, textarea, select, [role=dialog], [role=menu]"))
        return;
      if (event.key === "g") {
        event.preventDefault();
        setActivity({ kind: "record", day });
        return;
      }
      const amount = { ArrowLeft: -1, ArrowRight: 1 }[event.key];
      if (!amount) return;
      setParams({ date: dayParam(addDays(day, amount)) }, { replace: true });
    }
    document.addEventListener("keydown", step);
    return () => document.removeEventListener("keydown", step);
  }, [day, setParams]);

  // A reader coming back from a moment lands on it, once the cards have stopped growing out of
  // their placeholders. Scrolling, unlike a #fragment, leaves focus alone, so the title does not
  // light up with a focus ring. The target is used once and dropped.
  const target = params.get("moment");
  const settled = useCallback(() => {
    if (!target) return;
    document
      .getElementById(`moment-${target}`)
      ?.scrollIntoView({ block: "center" });
    setParams({ date }, { replace: true });
  }, [target, date, setParams]);

  return (
    <>
      <header className="day reveals-more">
        <Popover open={picking} onOpenChange={setPicking}>
          <h1>
            <PopoverTrigger asChild>
              <button
                type="button"
                className="date"
                // The heading must still read as the date, not only as a control.
                aria-label={`${dayHeading(day)}，选择日期`}
                onPointerEnter={() => void calendar.preload()}
                onFocus={() => void calendar.preload()}
                onPointerDown={() => (opened.current = "pointer")}
                onKeyDown={() => (opened.current = "keyboard")}
                // A click must not leave focus on the date, or the arrow keys that step days
                // would ring it, as with 时刻 in the header.
                onMouseDown={(event) => event.preventDefault()}
              >
                {dayHeading(day)}
              </button>
            </PopoverTrigger>
          </h1>
          <PopoverContent
            className="w-auto p-0"
            align="start"
            onCloseAutoFocus={(event) => {
              // Keyboard users go back to the date; a pointer never put focus there.
              if (opened.current === "pointer") event.preventDefault();
            }}
          >
            <Suspense fallback={null}>
              <DayCalendar
                selected={day}
                onSelect={(picked) => {
                  show(picked);
                  setPicking(false);
                }}
              />
            </Suspense>
          </PopoverContent>
        </Popover>
        <MoreMenu
          label="当天操作"
          onApproach={() => {
            setMenuTime(new Date());
            void importDialog.preload();
            void keyManager.preload();
          }}
          actions={[
            ...sleepActions,
            {
              label: "记录工夫",
              onSelect: () => setActivity({ kind: "record", day }),
            },
            {
              label: "编排日课",
              onSelect: () => setArranging(true),
              disabled: !habits.data,
            },
            {
              label: "日课与工夫",
              onSelect: () => navigate(`/habits?date=${date}`),
            },
            { label: "导入对话", onSelect: () => setImporting(true) },
            { label: "密钥管理", onSelect: () => setManagingKeys(true) },
          ]}
        />
        <HabitSeals day={day} />
      </header>
      {query.isError ? (
        <ErrorState error={query.error} retry={() => void query.refetch()} />
      ) : (
        <>
          <SleepPin day={day} isToday={isToday} expectsNight={expectsNight} />
          <DayMoments
            // A new day starts over: no leftover placeholders and no exit animation.
            key={date}
            outline={shapes}
            moments={cards}
            date={date}
            // A day whose only record is its night already shows the night.
            empty={expectsNight ? "blank" : "illustrated"}
            onSettled={settled}
            onOpenActivity={(moment) =>
              setActivity({
                kind: "edit",
                activity: {
                  id: moment.id,
                  type_id: moment.type_id,
                  type_name: moment.type_name,
                  started_at: moment.occurred_at,
                  minutes: moment.minutes,
                  note: moment.note,
                },
              })
            }
          />
        </>
      )}
      {importing && (
        <Suspense fallback={null}>
          <ImportDialog
            open={importing}
            onOpenChange={setImporting}
            mode={{ kind: "conversation", day }}
            onImported={(result, occurredAt) => {
              // Back from the new conversation, and its back link, both lead to the day it was
              // filed under, where the new card now is — not necessarily the day being viewed.
              const filed = dayParam(dayOf(occurredAt));
              navigate(`/?date=${filed}&moment=${result.path_id}`, {
                replace: true,
              });
              navigate(
                `/conversations/${result.conversation_id}?path=${result.path_id}&date=${filed}`,
              );
            }}
          />
        </Suspense>
      )}
      {recording && (
        <SleepNightDialog
          target={recording}
          usual={[
            (nights.data && averageClock(nights.data, "started_at")) ?? "23:30",
            (nights.data && averageClock(nights.data, "ended_at")) ?? "07:00",
          ]}
          onClose={() => setRecording(null)}
        />
      )}
      {activity && (
        <ActivityDialog target={activity} onClose={() => setActivity(null)} />
      )}
      {arranging && habits.data && (
        <HabitArrangeDialog
          habits={habits.data.habits.filter((habit) => !habit.retired)}
          onClose={() => setArranging(false)}
        />
      )}
      {managingKeys && (
        <Suspense fallback={null}>
          <KeyManager onClose={() => setManagingKeys(false)} />
        </Suspense>
      )}
    </>
  );
}
