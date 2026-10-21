import { useEffect, useRef, useState, type ReactNode } from "react";
import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { useSearchParams } from "react-router-dom";
import { jottingListOptions, jottingOverviewOptions } from "@/lib/api";
import { heatmapDays } from "@/lib/jotting-time";
import { EchoSide, TagFlow } from "@/components/echo/echo-side";
import { JottingComposer } from "@/components/echo/jotting-composer";
import { JottingWall } from "@/components/echo/jotting-wall";
import { RecallPile } from "@/components/echo/recall-pile";
import {
  modKey,
  useShortcutHelp,
  type ShortcutGroup,
} from "@/components/shortcut-help";

// 回响: every jotting on a wide masonry wall, with today's recall pile and the counts in the
// left column. `n` writes and `/` searches; neither has a visible entry. The tag filter and
// the search live in the address, so going back returns to them.
// See specs/decisions/web/echo/0-jotting-wall-with-daily-recall-pile.md.
const shortcuts: ShortcutGroup = {
  title: "回响",
  shortcuts: [
    { keys: ["n"], action: "写札记" },
    { keys: ["/"], action: "搜索札记" },
    { keys: ["Esc"], action: "合上回响卡片，退出全部标签或标签筛选" },
  ],
};
// The composer handles these itself; they are listed here because 回响 is where it lives.
const composerShortcuts: ShortcutGroup = {
  title: "写札记时",
  shortcuts: [
    { keys: [`${modKey} Enter`], action: "保存" },
    { keys: ["↑", "↓"], action: "选择标签建议" },
    { keys: ["Enter", "Tab"], action: "填入标签" },
    { keys: ["Esc"], action: "未写内容时收起" },
  ],
};

export function EchoPage() {
  const [params, setParams] = useSearchParams();
  const tag = params.get("tag") ?? "";
  const q = params.get("q") ?? "";
  const filtering = tag !== "" || q !== "";
  // The all-tags view replaces the wall; it is a glance, not a place to come back to.
  const [allTags, setAllTags] = useState(false);
  const [composing, setComposing] = useState(false);
  const [searching, setSearching] = useState(false);
  const [draft, setDraft] = useState(q);
  // Going back changes the search under the field; the field follows.
  const [shown, setShown] = useState(q);
  if (shown !== q) {
    setShown(q);
    setDraft(q);
  }
  const search = useRef<HTMLInputElement>(null);

  const overview = useQuery(jottingOverviewOptions(heatmapDays(new Date())[0]));
  const list = useInfiniteQuery(jottingListOptions(tag, q));
  const total = list.data?.pages[0]?.total;

  // Replace: refining a filter is still the same visit, back should leave 回响.
  function filter(next: { tag?: string; q?: string }) {
    const entries = Object.entries({ tag, q, ...next }).filter(([, v]) => v);
    setParams(Object.fromEntries(entries), { replace: true });
    window.scrollTo({ top: 0 });
  }
  // Choosing a tag anywhere lands on the wall filtered by it; the active one again clears it.
  function pickTag(name: string) {
    filter({ tag: !allTags && tag === name ? "" : name });
    setAllTags(false);
  }
  function closeSearch() {
    setSearching(false);
    search.current?.blur();
  }

  useShortcutHelp(shortcuts);
  useShortcutHelp(composerShortcuts);
  // Escape steps back one layer at a time; the recall pile takes it first while open.
  useEffect(() => {
    function key(event: KeyboardEvent) {
      if (event.metaKey || event.ctrlKey || event.altKey) return;
      const target = event.target as HTMLElement;
      if (
        target.closest(
          "input, textarea, select, [contenteditable], [role=dialog]",
        )
      )
        return;
      if (event.key === "Escape") {
        if (allTags) setAllTags(false);
        else if (tag) filter({ tag: "" });
      } else if (event.key === "n") {
        event.preventDefault();
        setSearching(false);
        setComposing(true);
      } else if (event.key === "/") {
        event.preventDefault();
        setSearching(true);
        search.current?.focus({ preventScroll: true });
      }
    }
    document.addEventListener("keydown", key);
    return () => document.removeEventListener("keydown", key);
  });

  let main: ReactNode;
  if (allTags)
    main = (
      <TagFlow
        overview={overview.data}
        activeTag={tag}
        filter={searching ? draft : ""}
        onTag={pickTag}
        onBack={() => setAllTags(false)}
      />
    );
  else
    main = (
      <>
        {filtering && (
          <p className="filter-line">
            <strong>
              {[tag && `#${tag}`, q && `“${q}”`].filter(Boolean).join(" · ")}
            </strong>
            {total !== undefined && <span>{total} 张</span>}
            <button
              type="button"
              onClick={() => {
                setDraft("");
                closeSearch();
                filter({ tag: "", q: "" });
              }}
            >
              清除
            </button>
          </p>
        )}
        <JottingWall tag={tag} q={q} onTag={pickTag} />
      </>
    );

  return (
    <div className="echo">
      <aside className="side">
        <Drawer open={!filtering} settles>
          <RecallPile away={filtering} />
        </Drawer>
        <EchoSide
          overview={overview.data}
          activeTag={tag}
          allTags={allTags}
          onTag={pickTag}
          onAllTags={() => setAllTags(!allTags)}
        />
      </aside>
      <div className="echo-main">
        <Drawer open={composing}>
          <JottingComposer
            open={composing}
            knownTags={overview.data?.tags.map((t) => t.name) ?? []}
            onClose={() => setComposing(false)}
            onCreated={() => {
              setComposing(false);
              setAllTags(false);
              // The new jotting might not match the filter; showing everything shows it.
              if (filtering) {
                setDraft("");
                closeSearch();
                filter({ tag: "", q: "" });
              }
            }}
          />
        </Drawer>
        <Drawer open={searching || q !== ""}>
          <input
            ref={search}
            className="search"
            type="search"
            aria-label="搜索札记"
            placeholder={allTags ? "筛选标签" : "搜索札记"}
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            onKeyDown={(event) => {
              // Only a plain Enter searches: typing, an IME's own Enter included, leaves the
              // wall alone. The all-tags view filters as you type instead.
              if (event.nativeEvent.isComposing) return;
              if (event.key === "Enter" && !allTags) {
                filter({ q: draft.trim() });
              } else if (event.key === "Escape") {
                // The first Escape empties the field, the next one puts it away.
                if (draft) setDraft("");
                else closeSearch();
                if (q) filter({ q: "" });
              }
            }}
          />
        </Drawer>
        {main}
      </div>
    </div>
  );
}

// A row that slides open and shut. Shut, it is inert so its field cannot take focus. With
// `settles`, an open drawer stops clipping once it has slid open, so the spread recall pile can
// reach across the wall.
function Drawer({
  open,
  settles,
  children,
}: {
  open: boolean;
  settles?: boolean;
  children: ReactNode;
}) {
  const [settled, setSettled] = useState(open);
  if (!open && settled) setSettled(false);
  const className = [
    "drawer",
    !open && "closed",
    settles && settled && "settled",
  ]
    .filter(Boolean)
    .join(" ");
  return (
    <div
      className={className}
      inert={!open}
      onTransitionEnd={(event) => {
        if (open && event.target === event.currentTarget) setSettled(true);
      }}
    >
      {/* overflow: clip, not hidden; see the .drawer rule in echo.css. */}
      <div>{children}</div>
    </div>
  );
}
