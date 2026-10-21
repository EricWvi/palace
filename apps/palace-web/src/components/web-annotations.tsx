import { useEffect, useState, type CSSProperties } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  api,
  apiData,
  annotationsOptions,
  webAnnotationsOptions,
  type Annotation,
  type WebAnnotation,
} from "@/lib/api";
import { scopeSelector, scopeTexts } from "@/lib/annotation-dom";
import { relocate } from "@/lib/text-quote";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { articleScope } from "./annotation-layer";
import { swatch } from "./note-card";

const none: WebAnnotation[] = [];

// Moves the annotations the browser extension left on pages bound to this article into the
// article. Only the rendered body has the coordinates an article annotation needs, so once the
// body is on screen each web annotation is looked for in its annotation text by quote and
// context, and every one found is swapped for an article annotation in one request. The rest
// stay, listed for the reader to redo by hand or discard; the body never changes, so they are
// never tried again. `ready` says the body is rendered.
// See specs/decisions/extension/0-web-page-annotations-bound-and-migrated-into-articles.md (D5).
export function useWebAnnotations(article: string, ready: boolean) {
  const client = useQueryClient();
  const options = webAnnotationsOptions(article);
  const query = useQuery(options);
  const items = query.data?.items ?? none;
  // What became of each annotation looked for on this page, so none is looked for twice and a
  // failed request never loops; one moving is not listed while its request runs.
  const [status, setStatus] = useState<
    ReadonlyMap<string, "moving" | "unmatched">
  >(new Map());
  const [error, setError] = useState<string | null>(null);

  const drop = (gone: ReadonlySet<string>) =>
    client.setQueryData(
      options.queryKey,
      (old: { items: WebAnnotation[] } | undefined) =>
        old && { items: old.items.filter((item) => !gone.has(item.id)) },
    );

  useEffect(() => {
    const fresh = items.filter((item) => !status.has(item.id));
    const root = document.querySelector(
      `${scopeSelector}[data-annotation-scope="${articleScope}"]`,
    );
    if (!ready || fresh.length === 0 || !root) return;
    const text = scopeTexts(root)
      .map((node) => node.data)
      .join("");
    const found = fresh.flatMap((item) => {
      const at = relocate(text, item);
      return at ? [{ web_annotation_id: item.id, ...at }] : [];
    });
    const sent = new Set(found.map((item) => item.web_annotation_id));
    const mark = (outcome: (id: string) => "moving" | "unmatched") =>
      setStatus((current) => {
        const next = new Map(current);
        for (const item of fresh) next.set(item.id, outcome(item.id));
        return next;
      });
    // The text exists only once the body is on screen, so the outcome is known only here.
    mark((id) => (sent.has(id) ? "moving" : "unmatched"));
    if (found.length === 0) return;
    apiData(
      api.POST("/api/articles/{id}/web-annotations/migrate", {
        params: { path: { id: article } },
        body: { items: found },
      }),
    )
      .then((created) => {
        client.setQueryData(
          annotationsOptions({ kind: "article", id: article }).queryKey,
          (old: { items: Annotation[] } | undefined) =>
            old && { items: [...old.items, ...created.items] },
        );
        // Whatever was sent is gone from the pages now: moved here, or by another tab first.
        drop(sent);
      })
      .catch((failure: unknown) => {
        mark(() => "unmatched");
        setError(
          `原网页标注没有迁移：${failure instanceof Error ? failure.message : "请刷新后重试。"}`,
        );
      });
    // `drop` and the client are stable for one article; the items and readiness drive this.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [items, ready, article, status]);

  const discard = (id: string) => {
    setError(null);
    apiData(
      api.DELETE("/api/web-annotations/{id}", { params: { path: { id } } }),
    )
      .then(() => drop(new Set([id])))
      .catch((failure: unknown) =>
        setError(
          `没有丢弃：${failure instanceof Error ? failure.message : "请稍后重试。"}`,
        ),
      );
  };

  return {
    // Looked for and not found; those still being looked for or moved are not listed yet.
    unmatched: items.filter((item) => status.get(item.id) === "unmatched"),
    discard,
    error,
  };
}

export type WebAnnotations = ReturnType<typeof useWebAnnotations>;

// The meta line's word on web annotations that found no place in the body, opening their list:
// each quote with its style and note, to annotate again by hand, and a way to discard it.
export function UnmatchedFacts({ web }: { web: WebAnnotations }) {
  const count = web.unmatched.length;
  if (count === 0) return null;
  return (
    <Popover>
      <PopoverTrigger asChild>
        <button type="button">{count} 处原网页标注没对上</button>
      </PopoverTrigger>
      <PopoverContent className="unmatched w-96 p-0" align="start">
        <p className="unmatched-hint">
          正文里找不到这些原文，可以对照后重新标注。
        </p>
        <ul>
          {web.unmatched.map((item) => (
            <li
              key={item.id}
              style={{ "--swatch": swatch(item.style) } as CSSProperties}
            >
              <blockquote>{item.quote}</blockquote>
              {item.note !== null && (
                <p className="unmatched-note">{item.note}</p>
              )}
              <button type="button" onClick={() => web.discard(item.id)}>
                丢弃
              </button>
            </li>
          ))}
        </ul>
      </PopoverContent>
    </Popover>
  );
}
