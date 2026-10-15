import { useEffect, useLayoutEffect } from "react";
import { useQuery } from "@tanstack/react-query";
import { useLocation, useParams } from "react-router-dom";
import { ApiError, noteOptions } from "@/lib/api";
import { ErrorState } from "@/components/error-state";
import {
  MarkdownContent,
  type ImagePolicy,
} from "@/components/markdown-content";

// Notes are the owner's own writing, and their images are the owner's uploads: they load as
// written. Module-level, so the memoized content never sees a new policy object.
const own: ImagePolicy = { kind: "load", resolve: (src) => src };

// 摘星's reading page for one note. Notes are maintained outside Palace, so the page only reads:
// no menu, no actions, and no way back but the browser's, as for a conversation opened from 摘星.
export function NotePage() {
  // The id keeps its `/`, so the route hands it over as the rest of the address.
  const id = useParams()["*"] ?? "";
  const note = useQuery(noteOptions(id));
  const { state } = useLocation();
  const top = (state as { top?: boolean } | null)?.top;
  useLayoutEffect(() => {
    if (top) window.scrollTo(0, 0);
  }, [top, id]);
  const title = note.data?.title;
  useEffect(() => {
    if (title) document.title = title;
  }, [title]);

  if (note.isPending)
    return (
      <header className="note">
        <p role="status" className="sr-only">
          正在加载笔记…
        </p>
        <div className="skeleton" aria-hidden="true">
          <span className="bar head" />
          <span className="bar meta" />
        </div>
      </header>
    );
  if (note.isError)
    return (
      <ErrorState
        error={
          note.error instanceof ApiError && note.error.status === 404
            ? new ApiError(404, "找不到这篇笔记。")
            : note.error
        }
        retry={() => void note.refetch()}
      />
    );
  const date = new Date(note.data.occurred_at);
  return (
    <>
      <header className="note">
        <h1>{note.data.title}</h1>
        <p className="meta">
          <time dateTime={date.toISOString()}>{fullDate(date)}</time>
        </p>
      </header>
      <div className="note-body">
        <MarkdownContent content={note.data.content} images={own} />
      </div>
    </>
  );
}

// `YYYY.MM.DD` in the reader's time zone.
function fullDate(date: Date) {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${date.getFullYear()}.${pad(date.getMonth() + 1)}.${pad(date.getDate())}`;
}
