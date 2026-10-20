import { useEffect, useLayoutEffect } from "react";
import { useQuery } from "@tanstack/react-query";
import { useLocation, useParams } from "react-router-dom";
import { ApiError, noteOptions } from "@/lib/api";
import { ErrorState } from "@/components/error-state";
import { type ImagePolicy } from "@/components/markdown-content";
import {
  ReadingDocument,
  ReadingSkeleton,
  fullDate,
} from "@/components/reading-document";

// Notes are the owner's own writing, and their images are the owner's uploads: they load as
// written. Module-level, so the memoized content never sees a new policy object.
const own: ImagePolicy = {
  kind: "load",
  resolve: (src) => ({ kind: "image", src }),
};

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

  if (note.isPending) return <ReadingSkeleton label="正在加载笔记…" />;
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
    <ReadingDocument
      id={id}
      title={note.data.title}
      content={note.data.content}
      images={own}
      meta={<time dateTime={date.toISOString()}>{fullDate(date)}</time>}
      tocLabel="笔记目录"
    />
  );
}
