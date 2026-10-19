import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { useInfiniteQuery } from "@tanstack/react-query";
import { Link, useLocation, useSearchParams } from "react-router-dom";
import {
  conversationListOptions,
  noteListOptions,
  notePath,
  sources,
} from "@/lib/api";
import { ErrorState } from "@/components/error-state";

// The kinds that can be opened, each with its own list address; 文章 is not built yet.
export type Kind = "notes" | "conversations";
const kinds: Record<Kind, { name: string; to: string }> = {
  notes: { name: "笔记", to: "/notes" },
  conversations: { name: "对话", to: "/conversations" },
};

// One line of the contents, whatever its kind.
type Row = {
  key: string;
  title: string;
  aside?: string;
  occurred_at: number;
  to: string;
};

// Where the reader had scrolled each visit of the list, by history entry, so going back from a
// conversation lands on the row they left. Module scope outlives the page's own unmount.
const offsets = new Map<string, number>();

// 摘星's entry: a contents page. The kinds stand in a column on the left; the current kind's list
// runs on the right, one entry per line, grouped by year. `/` opens a search that runs on Enter.
export function StarsPage({ kind }: { kind: Kind }) {
  const [params, setParams] = useSearchParams();
  const q = params.get("q") ?? "";
  // Both lists load, the other one unsearched: the kind column counts every kind it can open,
  // and switching kinds then shows a list already cached.
  const notes = useInfiniteQuery(noteListOptions(kind === "notes" ? q : ""));
  const conversations = useInfiniteQuery(
    conversationListOptions(kind === "conversations" ? q : ""),
  );
  const list = kind === "notes" ? notes : conversations;
  const rows: Row[] =
    kind === "notes"
      ? (notes.data?.pages ?? []).flatMap((page) =>
          page.items.map((note) => ({
            key: note.id,
            title: note.title,
            occurred_at: note.occurred_at,
            to: notePath(note.id),
          })),
        )
      : (conversations.data?.pages ?? []).flatMap((page) =>
          page.items.map((path) => ({
            key: path.id,
            title: path.title,
            aside: sources[path.source],
            occurred_at: path.occurred_at,
            to: `/conversations/${path.conversation_id}?path=${path.id}`,
          })),
        );
  const totals: Record<Kind, number | undefined> = {
    notes: notes.data?.pages[0]?.total,
    conversations: conversations.data?.pages[0]?.total,
  };
  const { name } = kinds[kind];
  const { key } = useLocation();

  const [searching, setSearching] = useState(false);
  const [draft, setDraft] = useState(q);
  // Going back or following the 对话 link changes the search under the field; the field follows.
  const [shown, setShown] = useState(q);
  if (shown !== q) {
    setShown(q);
    setDraft(q);
  }
  const search = useRef<HTMLInputElement>(null);
  // A search in the address keeps its field open, also after going back to it.
  const showSearch = searching || q !== "";

  function runSearch(value: string) {
    // Replace: refining a search is still the same visit, back should leave 摘星.
    const term = value.trim();
    setParams(term ? { q: term } : {}, { replace: true });
  }

  useEffect(() => {
    function open(event: KeyboardEvent) {
      if (event.key !== "/" || event.metaKey || event.ctrlKey || event.altKey)
        return;
      const target = event.target as HTMLElement;
      if (
        target.closest(
          "input, textarea, select, [contenteditable], [role=dialog]",
        )
      )
        return;
      event.preventDefault();
      setSearching(true);
      search.current?.focus();
    }
    document.addEventListener("keydown", open);
    return () => document.removeEventListener("keydown", open);
  }, []);
  // The field is mounted hidden, so `/` can focus it in the same keystroke that reveals it.
  useEffect(() => {
    if (searching) search.current?.focus();
  }, [searching]);

  // Remember the offset as the reader scrolls, and stop once a row is opened: the reading page
  // moves the window before this page has finished unmounting.
  const leaving = useRef(false);
  useEffect(() => {
    let frame = 0;
    function remember() {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        if (!leaving.current) offsets.set(key, window.scrollY);
      });
    }
    window.addEventListener("scroll", remember, { passive: true });
    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener("scroll", remember);
    };
  }, [key]);
  // Restore only when the rows are already cached on arrival: a list loaded afresh (after an
  // edit dropped the cache) has different rows, and an old offset would land on a stranger.
  const restorable = useRef(list.data !== undefined);
  useLayoutEffect(() => {
    if (!restorable.current) return;
    restorable.current = false;
    const offset = offsets.get(key);
    if (offset !== undefined) window.scrollTo(0, offset);
  }, [key]);

  // The next page loads as the end of the list comes into view.
  const end = useRef<HTMLDivElement>(null);
  const { hasNextPage, isFetchingNextPage, fetchNextPage } = list;
  useEffect(() => {
    const target = end.current;
    if (!target || !hasNextPage || typeof IntersectionObserver === "undefined")
      return;
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting) && !isFetchingNextPage)
        void fetchNextPage();
    });
    observer.observe(target);
    return () => observer.disconnect();
  }, [hasNextPage, isFetchingNextPage, fetchNextPage]);

  let body: ReactNode;
  if (list.isPending)
    body = (
      <p role="status" className="sr-only">
        正在加载{name}…
      </p>
    );
  else if (list.isError)
    body = <ErrorState error={list.error} retry={() => void list.refetch()} />;
  else if (rows.length === 0)
    // An empty kind shows nothing; only a search that misses says so.
    body = q ? <p className="empty">没有找到。</p> : null;
  else
    body = (
      <>
        {byYear(rows).map(([year, entries]) => (
          <section className="year" key={year}>
            <h2>{year}</h2>
            <ol>
              {entries.map((row) => (
                <li key={row.key}>
                  <Link
                    className="entry"
                    to={row.to}
                    // The window outlives the page; without this the entry would open as far
                    // down as the row was.
                    state={{ top: true }}
                    onClick={() => (leaving.current = true)}
                  >
                    <span className="entry-title">
                      {highlight(row.title, q)}
                    </span>
                    {row.aside && (
                      <span className="entry-aside">{row.aside}</span>
                    )}
                    <span className="leader" aria-hidden="true" />
                    <time
                      className="entry-date"
                      dateTime={new Date(row.occurred_at).toISOString()}
                    >
                      {monthDay(row.occurred_at)}
                    </time>
                  </Link>
                </li>
              ))}
            </ol>
          </section>
        ))}
        <div ref={end} />
      </>
    );

  return (
    <div className="contents">
      {/* Articles are not built yet: shown as a word, never a link or a tab stop. */}
      <nav className="kinds" aria-label="摘星分类">
        <KindLink kind="notes" current={kind} total={totals.notes} />
        <span>文章</span>
        <KindLink
          kind="conversations"
          current={kind}
          total={totals.conversations}
        />
      </nav>
      <div>
        <input
          ref={search}
          className="search"
          type="search"
          aria-label={`搜索${name}`}
          placeholder="搜索标题与正文"
          hidden={!showSearch}
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={(event) => {
            // An IME's own Enter confirms a candidate; only a plain Enter searches.
            if (event.key === "Enter" && !event.nativeEvent.isComposing) {
              runSearch(draft);
            } else if (event.key === "Escape") {
              // First Escape clears the search, the next one puts the field away.
              if (draft) {
                setDraft("");
                runSearch("");
              } else {
                runSearch("");
                setSearching(false);
                event.currentTarget.blur();
              }
            }
          }}
        />
        {body}
      </div>
    </div>
  );
}

function KindLink({
  kind,
  current,
  total,
}: {
  kind: Kind;
  current: Kind;
  total: number | undefined;
}) {
  const { name, to } = kinds[kind];
  return (
    <Link to={to} aria-current={kind === current ? "page" : undefined}>
      {name}
      {total !== undefined && <small>{total}</small>}
    </Link>
  );
}

// Groups rows under the local calendar year of their date, keeping list order.
function byYear(rows: Row[]) {
  const years: [number, Row[]][] = [];
  for (const row of rows) {
    const year = new Date(row.occurred_at).getFullYear();
    const last = years.at(-1);
    if (last?.[0] === year) last[1].push(row);
    else years.push([year, [row]]);
  }
  return years;
}

// `MM.DD` in the reader's time zone.
function monthDay(time: number) {
  const date = new Date(time);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(date.getMonth() + 1)}.${pad(date.getDate())}`;
}

// Marks every case-insensitive occurrence of the search term in a title.
function highlight(title: string, q: string): ReactNode {
  if (!q) return title;
  const lower = title.toLowerCase();
  const needle = q.toLowerCase();
  const parts: ReactNode[] = [];
  let from = 0;
  for (
    let at = lower.indexOf(needle);
    at >= 0;
    at = lower.indexOf(needle, from)
  ) {
    parts.push(
      title.slice(from, at),
      <mark key={at}>{title.slice(at, at + q.length)}</mark>,
    );
    from = at + q.length;
  }
  parts.push(title.slice(from));
  return parts;
}
