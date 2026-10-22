import { useMemo, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  api,
  apiData,
  annotationsOptions,
  type Annotation,
  type AnnotationTarget,
} from "@/lib/api";
import type { AnnotationStyle, Mark, Painting } from "@/lib/annotation-paint";
import type { MenuAction } from "./more-menu";

// An article's body is one scope; in a conversation every message is its own, named by its id.
export const articleScope = "article";
// The id a note being written takes before it is saved: a note needs words before it can exist.
export const draftId = "draft";

// Whether `id` names an annotation Palace has not saved yet: the note being written, or a new
// annotation drawn while its save is on the way. There is nothing to change on one until then.
export const unsaved = (id: string) => id.startsWith(draftId);

// A change drawn before the cached list holds it: what the annotation with this id is now, or
// null once it is gone. It starts as the change the reader made and becomes Palace's answer, and
// is dropped once the cache shows that answer. Saves finish in a microtask while the cache tells
// its readers a task later, so switching straight from one to the other would leave a moment
// showing neither, in which a margin with its only note would close and open again.
type Ahead = { id: string; shown: Annotation | null };

// The cached list with the changes drawn ahead of it; a new annotation goes at the end.
function ahead(list: Annotation[], changes: Ahead[]): Annotation[] {
  if (changes.length === 0) return list;
  const byId = new Map(changes.map((change) => [change.id, change.shown]));
  const shown = list
    .map((item) => (byId.has(item.id) ? byId.get(item.id)! : item))
    .filter((item) => item !== null);
  const known = new Set(list.map((item) => item.id));
  for (const change of changes)
    if (change.shown && !known.has(change.id)) shown.push(change.shown);
  return shown;
}

// Whether the cache already shows what `change` draws, so it can go.
const caughtUp = (list: Annotation[], change: Ahead) =>
  change.shown === null
    ? !list.some((item) => item.id === change.id)
    : list.includes(change.shown);

export type Anchor = {
  scope: string;
  start: number;
  end: number;
  quote: string;
};

const none: Annotation[] = [];
const nothingOpened: ReadonlySet<number> = new Set();

export const scopeOf = (annotation: Annotation) =>
  annotation.message_id ?? articleScope;

// The annotations of one reading page and everything the page does with them: drawing them,
// saving changes, a note still being written, and 只看标注 with the runs the reader opened.
export function useAnnotations(target: AnnotationTarget) {
  const client = useQueryClient();
  const options = annotationsOptions(target);
  const query = useQuery(options);
  const cached = query.data?.items ?? none;
  const [changes, setChanges] = useState<Ahead[]>([]);
  const list = useMemo(() => ahead(cached, changes), [cached, changes]);
  // Each time the cache moves, the changes it now shows go, so they never mask a later refetch.
  const [seen, setSeen] = useState(cached);
  if (seen !== cached) {
    setSeen(cached);
    setChanges((current) =>
      current.filter((change) => !caughtUp(cached, change)),
    );
  }
  const [draft, setDraft] = useState<Anchor | null>(null);
  const [focus, setFocusState] = useState(false);
  const [opened, setOpened] = useState<
    ReadonlyMap<string, ReadonlySet<number>>
  >(new Map());
  const [error, setError] = useState<string | null>(null);

  // One painting per scope, rebuilt only when what it draws changes, so each memoized body
  // re-renders only when its own marks or folding do.
  const paintings = useMemo(() => {
    const marks = new Map<string, Mark[]>();
    const add = (scope: string, mark: Mark) =>
      marks.set(scope, [...(marks.get(scope) ?? []), mark]);
    for (const annotation of list)
      add(scopeOf(annotation), {
        id: annotation.id,
        start: annotation.start,
        end: annotation.end,
        quote: annotation.quote,
        style: annotation.style,
        noted: annotation.note !== null,
      });
    if (draft)
      add(draft.scope, { id: draftId, ...draft, style: null, noted: true });
    const result = new Map<string, Painting>();
    for (const [scope, scoped] of marks)
      result.set(scope, {
        marks: scoped,
        fold: focus ? { expanded: opened.get(scope) ?? nothingOpened } : null,
      });
    return result;
  }, [list, draft, focus, opened]);

  // Applies a change to the cached list, so the page shows what the server now holds.
  const write = (change: (items: Annotation[]) => Annotation[]) =>
    client.setQueryData(
      options.queryKey,
      (old: { items: Annotation[] } | undefined) =>
        old ? { items: change(old.items) } : old,
    );
  // Draws `shown` for `id` at once and saves it: Palace's answer replaces it, a failed save puts
  // the page back as it was and says why once, at the foot of the page.
  async function saving(
    id: string,
    shown: Annotation | null,
    work: () => Promise<Annotation | null>,
  ) {
    const change: Ahead = { id, shown };
    setChanges((current) => [
      ...current.filter((other) => other.id !== id),
      change,
    ]);
    setError(null);
    try {
      const saved = await work();
      const answer: Ahead = { id: saved?.id ?? id, shown: saved };
      setChanges((current) =>
        current.map((other) => (other === change ? answer : other)),
      );
      write((items) =>
        saved === null
          ? items.filter((item) => item.id !== id)
          : items.some((item) => item.id === saved.id)
            ? items.map((item) => (item.id === saved.id ? saved : item))
            : [...items, saved],
      );
    } catch (failure) {
      setChanges((current) => current.filter((other) => other !== change));
      setError(
        `标注没有保存：${failure instanceof Error ? failure.message : "请稍后重试。"}`,
      );
    }
  }
  // Names each new annotation until Palace does.
  const unsavedCount = useRef(0);

  function create(
    anchor: Anchor,
    style: AnnotationStyle | null,
    note: string | null,
  ) {
    const body = {
      start: anchor.start,
      end: anchor.end,
      quote: anchor.quote,
      style,
      note,
    };
    const message_id = anchor.scope === articleScope ? null : anchor.scope;
    const now = Date.now();
    const id = `${draftId}-${++unsavedCount.current}`;
    return saving(
      id,
      { ...body, id, message_id, created_at: now, updated_at: now },
      () =>
        apiData(
          target.kind === "article"
            ? api.POST("/api/articles/{id}/annotations", {
                params: { path: { id: target.id } },
                body,
              })
            : api.POST("/api/conversations/{id}/annotations", {
                params: { path: { id: target.id } },
                body: { ...body, message_id: anchor.scope },
              }),
        ),
    );
  }

  // A deleted annotation stays drawn until Palace confirms it is gone.
  function remove(id: string) {
    const item = list.find((other) => other.id === id);
    if (!item) return Promise.resolve();
    return saving(id, item, async () => {
      await apiData(
        api.DELETE("/api/annotations/{id}", { params: { path: { id } } }),
      );
      return null;
    });
  }

  // Replaces the style and the note together; with neither left, the annotation goes.
  function update(
    id: string,
    style: AnnotationStyle | null,
    note: string | null,
  ) {
    const item = list.find((other) => other.id === id);
    if (!item) return Promise.resolve();
    if (style === null && note === null) return remove(id);
    return saving(id, { ...item, style, note }, () =>
      apiData(
        api.PUT("/api/annotations/{id}", {
          params: { path: { id } },
          body: { style, note },
        }),
      ),
    );
  }

  // Holds the first mark on screen where it is while most of the page folds or unfolds.
  function setFocus(on: boolean) {
    const anchor = [
      ...document.querySelectorAll<HTMLElement>("mark.anno"),
    ].find((mark) => mark.getBoundingClientRect().bottom > 0);
    const id = anchor?.dataset.annotation;
    const before = anchor?.getBoundingClientRect().top ?? 0;
    setFocusState(on);
    if (!on) setOpened(new Map());
    if (id)
      requestAnimationFrame(() => {
        const after = document.querySelector(`mark[data-annotation="${id}"]`);
        if (after)
          window.scrollBy(0, after.getBoundingClientRect().top - before);
      });
  }
  const available = list.length > 0;
  // 只看标注 needs something to show; once in it, the way out always works.
  const toggleFocus = () => {
    if (focus || available) setFocus(!focus);
  };

  return {
    list,
    count: list.length,
    paintingFor: (scope: string) => paintings.get(scope),
    draft,
    setDraft,
    // A page widens for its margin only while some annotation carries a note.
    hasNotes: draft !== null || list.some((item) => item.note !== null),
    focus,
    toggleFocus,
    exitFocus: () => focus && setFocus(false),
    // Opens one folded run of a scope; the others stay folded.
    open: (scope: string, run: number) =>
      setOpened((current) =>
        new Map(current).set(
          scope,
          new Set([...(current.get(scope) ?? []), run]),
        ),
      ),
    opened: (scope: string) => opened.get(scope) ?? nothingOpened,
    // Every opened run, for whatever has to follow the page as runs open.
    openedRuns: opened,
    create,
    update,
    remove,
    error,
    menuAction: {
      label: focus ? "显示全文" : "只看标注",
      onSelect: toggleFocus,
      disabled: !focus && !available,
    } satisfies MenuAction,
  };
}

export type Annotations = ReturnType<typeof useAnnotations>;

// The end of a reading page's line of facts: how many annotations there are, or, while only
// they are shown, the way back to the full text.
export function AnnotationFacts({ layer }: { layer: Annotations }) {
  if (layer.focus)
    return (
      <>
        <span>只看 {layer.count} 处标注</span>
        <button type="button" onClick={layer.toggleFocus}>
          显示全文
        </button>
      </>
    );
  return layer.count > 0 ? <span>{layer.count} 处标注</span> : null;
}
