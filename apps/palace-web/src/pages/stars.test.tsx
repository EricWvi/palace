import { act, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { address, mountApp } from "@/test-app";
import type { ConversationListItem, Detail } from "@/lib/api";
import { requestPath, testRequest } from "@/lib/test-request";

// Local wall-clock times, so years and `MM.DD` do not depend on the test machine's zone.
const at = (year: number, month: number, day: number) =>
  new Date(year, month - 1, day, 12).getTime();
const row = (
  id: string,
  conversation: string,
  title: string,
  updated: number,
): ConversationListItem => ({
  id,
  conversation_id: conversation,
  title,
  source: "chatgpt",
  updated_at: updated,
});
const rows = [
  row("p2", "c1", "改去 Point Reyes", at(2026, 10, 7)),
  row("p3", "c2", "PostgreSQL advisory lock", at(2026, 10, 5)),
  row("p1", "c1", "计划周末出行", at(2025, 12, 30)),
];
function detail(paths: ConversationListItem[]): Detail {
  return {
    conversation: { id: "c1", owner_id: "owner", source: "chatgpt" },
    messages: [
      {
        id: "m1",
        role: "user",
        content: "想去海边",
        owner_id: "owner",
        conversation_id: "c1",
        parent_message_id: null,
        toc_line: "想去海边",
        created_order: 1,
      },
    ],
    paths: paths.map((path) => ({
      id: path.id,
      title: path.title,
      session_id: path.id,
      head_message_id: "m1",
      occurred_at: path.updated_at,
      created_at: path.updated_at,
      updated_at: path.updated_at,
      message_count: 1,
      original_link: `https://chatgpt.com/c/${path.id}`,
    })),
  };
}

// Serves the list from `listed`, filtering by title the way a search would, and records every
// list request's query so tests can tell what reached the server and when.
function serve() {
  let listed = rows;
  const queries: Record<string, string>[] = [];
  vi.spyOn(globalThis, "fetch").mockImplementation(async (url) => {
    const request = testRequest(url);
    const path = requestPath(url);
    if (path === "/api/conversations") {
      const query = Object.fromEntries(new URL(request.url).searchParams);
      queries.push(query);
      const q = query.q?.toLowerCase();
      return Response.json({
        items: q
          ? listed.filter((item) => item.title.toLowerCase().includes(q))
          : listed,
        next_cursor: null,
        total: listed.length,
      });
    }
    if (request.method === "DELETE") {
      listed = listed.filter((item) => item.conversation_id !== "c1");
      return Response.json({ id: "c1" });
    }
    if (path.startsWith("/api/timeline")) return Response.json([]);
    return Response.json(
      detail(listed.filter((item) => item.conversation_id === "c1")),
    );
  });
  return { queries };
}
const scrolled = vi.fn();
beforeEach(() => {
  // jsdom implements no scrolling; the reading page asks for the top when opened from the list.
  window.scrollTo = scrolled as typeof window.scrollTo;
  scrolled.mockReset();
});
const titles = () =>
  [...document.querySelectorAll(".entry-title")].map(
    (title) => title.textContent,
  );

// Core test case: `specs/test-cases/web/stars/contents-page.md#stars-navigation-must-open-the-conversation-list-and-keep-unopened-kinds-inert`
it("opens the conversation list from 摘星, where only 对话 is open among the kinds", async () => {
  serve();
  const { router, user } = mountApp("/?date=2025-09-30");
  await user.click(await screen.findByRole("link", { name: "摘星" }));
  expect(address(router)).toBe("/conversations");
  // The page loads lazily; the bar moves to 摘星 together with it.
  const kinds = await screen.findByRole("navigation", { name: "摘星分类" });
  const top = screen.getByRole("navigation", { name: "主导航" });
  expect(
    [...top.querySelectorAll("[aria-current]")].map((item) => item.textContent),
  ).toEqual(["摘星"]);

  await waitFor(() =>
    expect([...kinds.children].map((kind) => kind.textContent)).toEqual([
      "笔记",
      "文章",
      "对话3",
    ]),
  );
  expect(within(kinds).getAllByRole("link")).toEqual([
    within(kinds).getByRole("link", { name: "对话3" }),
  ]);
  expect(within(kinds).getByRole("link")).toHaveAttribute(
    "aria-current",
    "page",
  );
  // Tabbing through the page goes from the top bar straight to 对话, past the unopened kinds.
  for (const name of ["时刻", "摘星", "对话3"]) {
    await user.tab();
    expect(document.activeElement).toHaveAccessibleName(name);
  }
});

// Core test case: `specs/test-cases/web/stars/contents-page.md#every-row-must-open-the-path-whose-title-and-date-it-shows`
it("lists each path on its own line by year and opens exactly that path at the top", async () => {
  serve();
  const { router, user } = mountApp("/conversations");
  await waitFor(() =>
    expect(titles()).toEqual([
      "改去 Point Reyes",
      "PostgreSQL advisory lock",
      "计划周末出行",
    ]),
  );
  expect(
    [...document.querySelectorAll(".year")].map((year) => [
      year.querySelector("h2")!.textContent,
      [...year.querySelectorAll(".entry")].map((entry) => [
        entry.querySelector(".entry-aside")!.textContent,
        entry.querySelector(".entry-date")!.textContent,
        entry.getAttribute("href"),
      ]),
    ]),
  ).toEqual([
    [
      "2026",
      [
        ["ChatGPT", "10.07", "/conversations/c1?path=p2"],
        ["ChatGPT", "10.05", "/conversations/c2?path=p3"],
      ],
    ],
    ["2025", [["ChatGPT", "12.30", "/conversations/c1?path=p1"]]],
  ]);
  // Two paths of one conversation are two rows, each opening its own path under its own title.
  await user.click(screen.getByRole("link", { name: /计划周末出行/ }));
  expect(
    await screen.findByRole("heading", { level: 1, name: "计划周末出行" }),
  ).toBeInTheDocument();
  expect(address(router)).toBe("/conversations/c1?path=p1");
  expect(screen.queryByRole("link", { name: /← / })).toBeNull();
  expect(scrolled).toHaveBeenCalledWith(0, 0);
});

// Core test case: `specs/test-cases/web/stars/contents-page.md#search-must-run-on-enter-and-match-only-the-paths-own-title-and-messages`
it("searches only on Enter, in place, and clears with Escape before putting the field away", async () => {
  const { queries } = serve();
  const { router, user } = mountApp("/conversations");
  await waitFor(() => expect(titles()).toHaveLength(3));
  expect(screen.getByRole("searchbox", { hidden: true })).not.toBeVisible();

  await user.keyboard("/");
  const field = screen.getByRole("searchbox", { name: "搜索对话" });
  expect(field).toHaveFocus();
  await user.type(field, "point");
  expect(queries).toEqual([{}]);
  await user.keyboard("{Enter}");
  await waitFor(() => expect(titles()).toEqual(["改去 Point Reyes"]));
  expect(queries).toEqual([{}, { q: "point" }]);
  expect([address(router), router.state.historyAction]).toEqual([
    "/conversations?q=point",
    "REPLACE",
  ]);
  expect(document.querySelector(".entry mark")).toHaveTextContent("Point");

  await user.clear(field);
  await user.type(field, "没有的词{Enter}");
  expect(await screen.findByText("没有找到。")).toBeInTheDocument();

  // A blank Enter is no search; Escape first clears a search, then hides the field.
  await user.clear(field);
  await user.type(field, "  {Enter}");
  await waitFor(() => expect(address(router)).toBe("/conversations"));
  await user.type(field, "lock{Enter}");
  await waitFor(() => expect(address(router)).toBe("/conversations?q=lock"));
  await user.keyboard("{Escape}");
  expect(address(router)).toBe("/conversations");
  expect(field).toHaveValue("");
  expect(field).toBeVisible();
  await user.keyboard("{Escape}");
  expect(field).not.toBeVisible();
  expect(field).not.toHaveFocus();
});

// Core test case: `specs/test-cases/web/stars/contents-page.md#returning-to-the-list-must-restore-the-search-and-drop-stale-rows`
it("comes back to the same search, and drops the rows of a conversation deleted meanwhile", async () => {
  const { queries } = serve();
  const { router, user } = mountApp("/conversations?q=e");
  await waitFor(() =>
    expect(titles()).toEqual(["改去 Point Reyes", "PostgreSQL advisory lock"]),
  );
  await user.click(screen.getByRole("link", { name: /改去 Point Reyes/ }));
  await screen.findByRole("heading", { level: 1, name: "改去 Point Reyes" });
  await act(() => router.navigate(-1));
  expect(address(router)).toBe("/conversations?q=e");
  expect(screen.getByRole("searchbox", { name: "搜索对话" })).toHaveValue("e");
  await waitFor(() => expect(titles()).toHaveLength(2));

  // Deleting from a page opened via 摘星 lands back on the list, replacing the deleted page.
  await user.click(screen.getByRole("link", { name: /改去 Point Reyes/ }));
  (await screen.findByRole("button", { name: "管理对话" })).focus();
  await user.keyboard("{Enter}");
  await user.click(screen.getByRole("menuitem", { name: "删除对话" }));
  const dialog = await screen.findByRole("dialog", { name: "删除对话？" });
  const before = queries.length;
  await user.click(within(dialog).getByRole("button", { name: "删除对话" }));
  await waitFor(() => expect(address(router)).toBe("/conversations"));
  expect(router.state.historyAction).toBe("REPLACE");
  await waitFor(() => expect(titles()).toEqual(["PostgreSQL advisory lock"]));
  expect(queries.slice(before)).toEqual([{}]);
});
