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

// The import form brings the calendar with it; most visits never open it.
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
  const pick = useRef<HTMLInputElement>(null);

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
        <h1>
          <button
            type="button"
            className="date"
            // The heading must still read as the date, not only as a control.
            aria-label={`${dayHeading(day)}，选择日期`}
            onClick={() => pick.current?.showPicker?.()}
          >
            {dayHeading(day)}
          </button>
        </h1>
        <MoreMenu
          label="当天操作"
          actions={[{ label: "导入对话", onSelect: () => setImporting(true) }]}
        />
        {/* The native input only supplies the picker; it stays invisible under the heading. */}
        <input
          ref={pick}
          className="pick"
          type="date"
          tabIndex={-1}
          aria-hidden="true"
          value={date}
          onChange={(event) => {
            const picked = parseDay(event.target.value);
            if (picked) show(picked);
          }}
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
