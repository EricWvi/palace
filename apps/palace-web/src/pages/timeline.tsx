import { lazy, Suspense, useEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Link, useNavigate, useSearchParams } from "react-router-dom";
import { sources, timelineOptions, type Moment } from "@/lib/api";
import {
  addDays,
  dayHeading,
  dayOf,
  dayParam,
  dayRange,
  parseDay,
  today,
} from "@/lib/day";
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
  const query = useQuery(timelineOptions(date, dayRange(day)));
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

  // A reader coming back from a moment lands on it. Scrolling, unlike a #fragment, leaves focus
  // alone, so the title does not light up with a focus ring. The target is used once and dropped.
  const target = params.get("moment");
  useEffect(() => {
    if (!target || !query.data) return;
    document
      .getElementById(`moment-${target}`)
      ?.scrollIntoView({ block: "center" });
    setParams({ date }, { replace: true });
  }, [target, query.data, date, setParams]);

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
      {query.isPending ? null : query.isError ? (
        <ErrorState error={query.error} retry={() => void query.refetch()} />
      ) : query.data.length === 0 ? (
        <p className="quiet-note">这一天还没有记录。</p>
      ) : (
        <ol className="timeline">
          {query.data.map((moment) => (
            <MomentCard key={moment.id} moment={moment} date={date} />
          ))}
        </ol>
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

// A conversation's full text belongs to 摘星, so its card only says what happened and links there.
function MomentCard({ moment, date }: { moment: Moment; date: string }) {
  const source = sources[moment.source];
  return (
    <li className="moment" id={`moment-${moment.id}`}>
      <Link
        className="moment-head"
        to={`/conversations/${moment.conversation_id}?path=${moment.id}&date=${date}`}
      >
        <span className="kind">对话</span>
        <span className="title">{moment.title}</span>
      </Link>
      <p className="meta">
        <span>{source}</span>
        <span>{moment.message_count} 条消息</span>
      </p>
      {moment.excerpt.length > 0 && (
        <blockquote className="chat">
          {moment.excerpt.map((line, index) => (
            <p key={index}>
              {line.role === "user" ? "你" : source}：{line.text}
            </p>
          ))}
        </blockquote>
      )}
    </li>
  );
}
