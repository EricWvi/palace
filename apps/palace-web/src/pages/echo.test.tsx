import {
  act,
  fireEvent,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { address, mountApp } from "@/test-app";
import type { JottingOverview, ListedJotting } from "@/lib/api";
import { bareDay, requestPath, testRequest } from "@/lib/test-request";

const at = (day: number, hour = 9) => new Date(2026, 8, day, hour).getTime();
const jotting = (
  id: string,
  body: string,
  tags: string[],
  created: number,
  extra: Partial<ListedJotting> = {},
): ListedJotting => ({
  id,
  created_at: created,
  title: null,
  tags,
  body,
  pending: false,
  ...extra,
});
const wall = [
  jotting("w1", "夜跑后的一点念头", ["跑步", "生活"], at(20), {
    pending: true,
  }),
  jotting("w2", "Arc 写多了要警惕", ["rust"], at(19), { title: "所有权" }),
  jotting("w3", "坂本龙一的 Aqua", ["音乐"], at(18)),
];
const pile = [1, 2, 3, 4, 5].map((n) =>
  jotting(`r${n}`, `回响第${n}张`, ["旧"], at(n)),
);
// Ten tags: `长尾` is the least used, outside the sidebar's top eight.
const overview: JottingOverview = {
  total: 3,
  days: 3,
  tags: [
    ...[
      "生活",
      "rust",
      "跑步",
      "音乐",
      "阅读",
      "阅历",
      "设计",
      "产品",
      "想法",
    ].map((name, i) => ({ name, count: 20 - i })),
    { name: "长尾", count: 1 },
  ],
  daily: [],
};

// Serves the 回响 reads and records every list query and upload, so tests can tell what reached
// the server and when. An upload joins the wall.
function serve() {
  let listed = [...wall];
  const queries: Record<string, string>[] = [];
  const uploads: Record<string, unknown>[] = [];
  vi.spyOn(globalThis, "fetch").mockImplementation(async (url) => {
    const request = testRequest(url);
    const path = requestPath(url);
    if (path === "/api/jottings" && request.method === "POST") {
      const body = (await request.json()) as Record<string, unknown>;
      uploads.push(body);
      listed = [
        jotting(
          body.id as string,
          body.body as string,
          body.tags as string[],
          Date.now(),
          {
            pending: true,
          },
        ),
        ...listed,
      ];
      return Response.json({ id: body.id, version: "9" }, { status: 201 });
    }
    if (path === "/api/jottings") {
      const query = Object.fromEntries(new URL(request.url).searchParams);
      queries.push(query);
      const jottings = listed.filter(
        (item) =>
          (!query.tag || item.tags.includes(query.tag)) &&
          (!query.q || item.body.includes(query.q)),
      );
      return Response.json({
        jottings,
        total: jottings.length,
        next_cursor: null,
      });
    }
    if (path === "/api/jottings/overview") return Response.json(overview);
    if (path === "/api/jottings/recall")
      return Response.json({ date: "2026-10-20", jottings: pile });
    throw new Error(`unexpected request ${path}`);
  });
  return { queries, uploads };
}

beforeEach(() => {
  vi.restoreAllMocks();
  // jsdom has no layout to scroll; a filter scrolls back to the top.
  window.scrollTo = vi.fn() as typeof window.scrollTo;
});
afterEach(() => {
  vi.restoreAllMocks();
});

const recall = () => screen.getByRole("region", { name: "今日回响" });
const filterLine = () =>
  screen.getByRole("button", { name: "清除" }).parentElement!;
const recallCards = () =>
  within(recall())
    .getAllByRole("button")
    .map((card) => card.dataset.id);

// specs/test-cases/web/echo/echo-page.md#echo-navigation-must-open-the-wide-jotting-page
it("widens the sheet for 回响 only and marks 回响 as the current section", async () => {
  serve();
  const { router, user } = mountApp("/jottings");
  await screen.findByText("夜跑后的一点念头");
  const nav = screen.getByRole("navigation", { name: "主导航" });
  expect(within(nav).getByRole("link", { name: "回响" })).toHaveAttribute(
    "aria-current",
    "page",
  );
  expect(screen.getByRole("main")).toHaveClass("sheet", "wide");
  // 时刻 asks for its day; it has nothing on it.
  vi.spyOn(globalThis, "fetch").mockImplementation(
    async (url) => bareDay(requestPath(url)) ?? Response.json([]),
  );
  await user.click(within(nav).getByRole("link", { name: "时刻" }));
  await waitFor(() => expect(address(router)).toBe("/"));
  expect(screen.getByRole("main")).not.toHaveClass("wide");
});

// specs/test-cases/web/echo/echo-page.md#read-recall-cards-must-leave-the-pile-for-the-page-session
it("lets each read recall card leave the pile until the page is opened again", async () => {
  serve();
  const stored = vi.spyOn(Storage.prototype, "setItem");
  const { router, user } = mountApp("/jottings");
  await screen.findByText("回响第1张");
  expect(recallCards()).toEqual(["r1", "r2", "r3", "r4", "r5"]);

  // The click that puts the spread pile away does not also filter by the tag under it.
  await user.click(within(recall()).getAllByRole("button")[0]);
  expect(recall()).toHaveClass("spread");
  await user.click(screen.getByRole("button", { name: "#rust" }));
  expect(recall()).not.toHaveClass("spread");
  expect(address(router)).toBe("/jottings");

  // Spread, read one, and Escape steps back a layer at a time.
  await user.click(within(recall()).getAllByRole("button")[0]);
  await user.click(within(recall()).getAllByRole("button")[1]);
  expect(
    within(screen.getByRole("dialog")).getByText("回响第2张"),
  ).toBeInTheDocument();
  await user.keyboard("{Escape}");
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(recallCards()).toEqual(["r1", "r3", "r4", "r5"]);
  expect(recall()).toHaveClass("spread");
  await user.keyboard("{Escape}");
  expect(recall()).not.toHaveClass("spread");

  // Reading the rest by clicking the backdrop leaves one quiet line.
  for (const id of ["r1", "r3", "r4", "r5"]) {
    const card = within(recall())
      .getAllByRole("button")
      .find((element) => element.dataset.id === id)!;
    if (!recall().classList.contains("spread")) await user.click(card);
    await user.click(card);
    await user.click(document.querySelector(".zoom-backdrop")!);
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  }
  expect(screen.getByText("今天的回响都已看过")).toBeInTheDocument();

  // Reading was never written down: coming back shows the whole pile again.
  // 时刻 asks for its day; it has nothing on it.
  vi.spyOn(globalThis, "fetch").mockImplementation(
    async (url) => bareDay(requestPath(url)) ?? Response.json([]),
  );
  await act(() => router.navigate("/"));
  serve();
  await act(() => router.navigate("/jottings"));
  await waitFor(() =>
    expect(recallCards()).toEqual(["r1", "r2", "r3", "r4", "r5"]),
  );
  expect(stored).not.toHaveBeenCalled();

  // A filter or a search puts the pile away; clearing brings it back as it was.
  const drawer = () => recall().closest(".drawer");
  await user.click(screen.getByRole("button", { name: "#rust" }));
  await waitFor(() => expect(address(router)).toBe("/jottings?tag=rust"));
  expect(drawer()).toHaveAttribute("inert");
  await user.click(screen.getByRole("button", { name: "清除" }));
  await waitFor(() => expect(address(router)).toBe("/jottings"));
  expect(drawer()).not.toHaveAttribute("inert");
  await user.keyboard("/");
  await user.keyboard("Aqua{Enter}");
  await waitFor(() => expect(address(router)).toBe("/jottings?q=Aqua"));
  expect(drawer()).toHaveAttribute("inert");
  expect(recallCards()).toEqual(["r1", "r2", "r3", "r4", "r5"]);
});

// specs/test-cases/web/echo/echo-page.md#tag-filters-and-search-must-stay-visible-and-intersect
it("keeps the active tag visible, searches only on Enter and steps back one layer per Escape", async () => {
  const { queries } = serve();
  const { router, user } = mountApp("/jottings?tag=%E9%95%BF%E5%B0%BE");
  const side = await screen.findByRole("button", { name: "#长尾1" });
  expect(side).toHaveAttribute("aria-current", "true");
  expect(
    screen
      .getByRole("button", { name: "全部标签10" })
      .parentElement!.querySelectorAll("button"),
  ).toHaveLength(10);
  await screen.findByText("没有找到。");
  expect(filterLine()).toHaveTextContent("#长尾0 张清除");

  // Clicking the active tag clears it; another tag filters.
  await user.click(side);
  await waitFor(() => expect(address(router)).toBe("/jottings"));
  await user.click(await screen.findByRole("button", { name: "#rust19" }));
  await waitFor(() => expect(address(router)).toBe("/jottings?tag=rust"));

  // Typing, including an IME's Enter, sends nothing; a plain Enter searches within the tag.
  await user.keyboard("/");
  const field = screen.getByRole("searchbox", { name: "搜索札记" });
  expect(field).toHaveFocus();
  const before = queries.length;
  await user.type(field, "Arc");
  fireEvent.keyDown(field, { key: "Enter", isComposing: true });
  expect(queries.length).toBe(before);
  await user.keyboard("{Enter}");
  await waitFor(() => expect(address(router)).toBe("/jottings?tag=rust&q=Arc"));
  await waitFor(() =>
    expect(queries.at(-1)).toEqual({ tag: "rust", q: "Arc" }),
  );
  expect(filterLine()).toHaveTextContent("#rust · “Arc”1 张清除");

  // In the field, Escape empties it and then puts it away.
  await user.keyboard("{Escape}");
  await waitFor(() => expect(address(router)).toBe("/jottings?tag=rust"));
  expect(field).toHaveValue("");
  await user.keyboard("{Escape}");
  expect(field).not.toHaveFocus();

  // On the page, Escape leaves the all-tags view first, then the tag filter.
  await user.click(screen.getByRole("button", { name: "全部标签10" }));
  expect(
    await screen.findByRole("region", { name: "全部标签" }),
  ).toBeInTheDocument();
  await user.keyboard("{Escape}");
  expect(screen.queryByRole("region", { name: "全部标签" })).toBeNull();
  expect(address(router)).toBe("/jottings?tag=rust");
  await user.keyboard("{Escape}");
  await waitFor(() => expect(address(router)).toBe("/jottings"));
});

// specs/test-cases/web/echo/echo-page.md#composing-must-extract-inline-tags-and-keep-the-rest-of-the-body-intact
it("writes a jotting with inline tags, keeps a draft through Escape and shows it unfiltered", async () => {
  const { uploads } = serve();
  const { router, user } = mountApp("/jottings?tag=rust");
  await screen.findByText("Arc 写多了要警惕");

  await user.keyboard("n");
  const field = screen.getByRole("textbox", { name: "新札记" });
  expect(field).toHaveFocus();
  await user.type(field, "读到 #阅");
  const options = within(screen.getByRole("listbox", { name: "已有标签" }))
    .getAllByRole("option")
    .map((option) => option.textContent);
  expect(options).toEqual(["#阅读", "#阅历"]);
  await user.keyboard("{ArrowDown}{Enter}");
  await user.type(field, "第三章 C# 不算");
  expect(field).toHaveValue("读到 #阅历 第三章 C# 不算");

  // A draft with words survives Escape and leaving the field.
  await user.keyboard("{Escape}");
  await user.click(document.body);
  expect(screen.getByRole("textbox", { name: "新札记" })).toHaveValue(
    "读到 #阅历 第三章 C# 不算",
  );

  await user.click(field);
  await user.keyboard("{Control>}{Enter}{/Control}");
  await waitFor(() => expect(uploads).toHaveLength(1));
  expect(uploads[0]).toEqual({
    id: expect.stringMatching(/^[0-9a-f]{8}-[0-9a-f]{4}-7/),
    created_at: expect.any(Number),
    tags: ["阅历"],
    body: "读到 第三章 C# 不算",
  });
  // The filter would hide the new jotting, so it goes.
  await waitFor(() => expect(address(router)).toBe("/jottings"));
  const card = (await screen.findByText("读到 第三章 C# 不算")).closest(
    "article",
  )!;
  expect(within(card).getByLabelText("待整理")).toBeInTheDocument();
  expect(field).toHaveValue("");
});
