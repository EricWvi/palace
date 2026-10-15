import { screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { address, mountApp } from "@/test-app";
import type { NoteListItem } from "@/lib/api";
import { requestPath, testRequest } from "@/lib/test-request";

// Local wall-clock times, so years and dates do not depend on the test machine's zone.
const at = (year: number, month: number, day: number) =>
  new Date(year, month - 1, day, 12).getTime();
const notes: NoteListItem[] = [
  {
    id: "rust/advisory-lock",
    title: "Advisory lock 的用法",
    occurred_at: at(2026, 10, 6),
  },
  { id: "旅行/周末 计划", title: "周末", occurred_at: at(2025, 9, 30) },
];
const bodies: Record<string, string> = {
  "rust/advisory-lock": "## 加锁\n\n![图](https://s3.example.com/lock.png)\n",
  "旅行/周末 计划": "去海边。\n",
};

// Serves notes the way the API does, filtering by title for a search, and records the raw paths
// of note requests and the queries of list requests.
function serve() {
  const opened: string[] = [];
  const queries: Record<string, string>[] = [];
  vi.spyOn(globalThis, "fetch").mockImplementation(async (url) => {
    const path = requestPath(url);
    if (path === "/api/notes") {
      const query = Object.fromEntries(
        new URL(testRequest(url).url).searchParams,
      );
      queries.push(query);
      const q = query.q?.toLowerCase();
      return Response.json({
        items: q
          ? notes.filter((note) => note.title.toLowerCase().includes(q))
          : notes,
        next_cursor: null,
        total: notes.length,
      });
    }
    if (path.startsWith("/api/notes/")) {
      opened.push(path);
      const id = decodeURIComponent(path.slice("/api/notes/".length));
      const note = notes.find((note) => note.id === id);
      return note
        ? Response.json({ ...note, content: bodies[id] })
        : Response.json({ error: "not_found", login: "" }, { status: 404 });
    }
    return Response.json({ items: [], next_cursor: null, total: 0 });
  });
  return { opened, queries };
}
const scrolled = vi.fn();
beforeEach(() => {
  // jsdom implements no scrolling; the note page asks for the top when opened from the list.
  window.scrollTo = scrolled as typeof window.scrollTo;
  scrolled.mockReset();
});

// Core test cases:
// - `specs/test-cases/web/stars/notes.md#note-list-must-page-every-indexed-note-exactly-once-by-its-date`
// - `specs/test-cases/web/stars/notes.md#note-page-must-read-only-files-listed-in-the-index`
// - `specs/test-cases/web/stars/notes.md#notes-must-be-read-only-and-readable-by-every-signed-in-user`
// - `specs/test-cases/web/stars/notes.md#stars-navigation-must-open-the-note-list-and-keep-articles-inert`
it("lists notes by date and opens one at the top, to read only", async () => {
  const { opened } = serve();
  const { router, user } = mountApp("/notes");
  await screen.findByText("Advisory lock 的用法");
  expect(
    [...document.querySelectorAll(".year")].map((year) => [
      year.querySelector("h2")!.textContent,
      [...year.querySelectorAll(".entry")].map((entry) => [
        entry.querySelector(".entry-title")!.textContent,
        entry.querySelector(".entry-aside")?.textContent ?? null,
        entry.querySelector(".entry-date")!.textContent,
        entry.getAttribute("href"),
      ]),
    ]),
  ).toEqual([
    [
      "2026",
      [["Advisory lock 的用法", null, "10.06", "/notes/rust/advisory-lock"]],
    ],
    [
      "2025",
      [
        [
          "周末",
          null,
          "09.30",
          "/notes/%E6%97%85%E8%A1%8C/%E5%91%A8%E6%9C%AB%20%E8%AE%A1%E5%88%92",
        ],
      ],
    ],
  ]);

  await user.click(screen.getByRole("link", { name: /周末/ }));
  expect(
    await screen.findByRole("heading", { level: 1, name: "周末" }),
  ).toBeInTheDocument();
  expect(address(router)).toBe(
    "/notes/%E6%97%85%E8%A1%8C/%E5%91%A8%E6%9C%AB%20%E8%AE%A1%E5%88%92",
  );
  // The id's `/` travels inside one encoded segment of the API path.
  expect(opened).toEqual([
    "/api/notes/%E6%97%85%E8%A1%8C%2F%E5%91%A8%E6%9C%AB%20%E8%AE%A1%E5%88%92",
  ]);
  expect(document.querySelector("header.note .meta")).toHaveTextContent(
    "2025.09.30",
  );
  expect(screen.getByText("去海边。")).toBeInTheDocument();
  expect(document.title).toBe("周末");
  expect(scrolled).toHaveBeenCalledWith(0, 0);
  // Nothing on the page changes a note, and 摘星 needs no way back but the browser's.
  expect(screen.queryAllByRole("button")).toEqual([]);
  expect(screen.queryByRole("link", { name: /← / })).toBeNull();
  expect(
    document.querySelector('nav[aria-label="主导航"] [aria-current]'),
  ).toHaveTextContent("摘星");
});

// Core test case: `specs/test-cases/web/stars/notes.md#note-page-must-read-only-files-listed-in-the-index`
it("shows the owner's images as written", async () => {
  serve();
  mountApp("/notes/rust/advisory-lock");
  expect(
    await screen.findByRole("heading", { level: 2, name: "加锁" }),
  ).toBeInTheDocument();
  expect(screen.getByRole("img", { name: "图" })).toHaveAttribute(
    "src",
    "https://s3.example.com/lock.png",
  );
});

// Core test case: `specs/test-cases/web/stars/notes.md#note-page-must-read-only-files-listed-in-the-index`
it("says when a note is not in the library", async () => {
  serve();
  mountApp("/notes/rust/gone");
  expect(await screen.findByRole("alert")).toHaveTextContent(
    "找不到这篇笔记。",
  );
});

// Core test case: `specs/test-cases/web/stars/notes.md#note-search-must-match-literally-and-stay-within-the-index`
it("searches notes on Enter with the shared search field", async () => {
  const { queries } = serve();
  const { router, user } = mountApp("/notes");
  await screen.findByText("Advisory lock 的用法");
  await user.keyboard("/");
  const field = screen.getByRole("searchbox", { name: "搜索笔记" });
  await user.type(field, "lock{Enter}");
  await waitFor(() => expect(address(router)).toBe("/notes?q=lock"));
  await waitFor(() =>
    expect(
      [...document.querySelectorAll(".entry-title")].map(
        (title) => title.textContent,
      ),
    ).toEqual(["Advisory lock 的用法"]),
  );
  expect(document.querySelector(".entry mark")).toHaveTextContent("lock");
  expect(queries).toEqual([{}, { q: "lock" }]);
});
