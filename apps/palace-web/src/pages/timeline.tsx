import {
  lazy,
  Suspense,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate, useSearchParams } from "react-router-dom";
import { outlineOptions, timelineOptions } from "@/lib/api";
import {
  addDays,
  dayHeading,
  dayOf,
  dayParam,
  dayRange,
  parseDay,
  today,
} from "@/lib/day";
import { DayMoments } from "@/components/day-moments";
import { ErrorState } from "@/components/error-state";
import { MoreMenu } from "@/components/more-menu";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";

// The calendar is fetched on first approach to the date, and the import form brings it along;
// most visits never open either.
const loadCalendar = () => import("@/components/day-calendar");
const DayCalendar = lazy(() =>
  loadCalendar().then((module) => ({ default: module.DayCalendar })),
);
const ImportDialog = lazy(() =>
  import("@/components/import-dialog").then((module) => ({
    default: module.ImportDialog,
  })),
);

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
  const [picking, setPicking] = useState(false);
  // How the calendar was opened decides where focus goes once it closes, see below.
  const opened = useRef<"pointer" | "keyboard">("keyboard");

  // Replace, never push: stepping through days is browsing, so back still leaves the timeline.
  function show(next: Date) {
    setParams({ date: dayParam(next) }, { replace: true });
  }

  useEffect(() => {
    function step(event: KeyboardEvent) {
      if (event.metaKey || event.ctrlKey || event.altKey) return;
      const target = event.target as HTMLElement;
      if (target.closest("input, textarea, select, [role=dialog], [role=menu]"))
        return;
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
                onPointerEnter={() => void loadCalendar()}
                onFocus={() => void loadCalendar()}
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
          actions={[{ label: "导入对话", onSelect: () => setImporting(true) }]}
        />
      </header>
      {query.isError ? (
        <ErrorState error={query.error} retry={() => void query.refetch()} />
      ) : (
        <DayMoments
          // A new day starts over: no leftover placeholders and no exit animation.
          key={date}
          outline={outline.data}
          moments={query.data}
          date={date}
          onSettled={settled}
        />
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
    </>
  );
}
