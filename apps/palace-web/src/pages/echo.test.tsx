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
import { absoluteTime } from "@/lib/jotting-time";
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
  version: "1",
  pending: false,
  recall: "included",
  ...extra,
});
const wall = [
  jotting("w1", "夜跑后的一点念头", ["跑步", "生活"], at(20), {
    pending: true,
  }),
  jotting("w2", "Arc 写多了要警惕", ["rust", "Vibe Coding"], at(19), {
    title: "所有权",
  }),
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

// What the server keeps between visits: which recall cards were completed.
type Kept = { done: Set<string> };

// Serves the 回响 reads and records every list query, upload and recall write, so tests can
// tell what reached the server and when. An upload joins the wall; a completed card stays done
// across visits when the same `kept` is passed again. `fail` makes the next completion fail;
// `conflict` answers the next edit or deletion with 409, `broken` with 500.
function serve(
  initial: ListedJotting[] = wall,
  kept: Kept = { done: new Set() },
) {
  let listed = [...initial];
  const queries: Record<string, string>[] = [];
  const uploads: Record<string, unknown>[] = [];
  const recalls: { path: string; body: unknown }[] = [];
  const writes: { method: string; path: string; body?: unknown }[] = [];
  const control = { fail: false, conflict: false, broken: false };
  vi.spyOn(globalThis, "fetch").mockImplementation(async (url) => {
    const request = testRequest(url);
    const path = requestPath(url);
    const completion = /^\/api\/jottings\/recall\/[\d-]+\/([^/]+)\/done$/.exec(
      path,
    );
    if (completion && request.method === "POST") {
      const body = (await request.json()) as { next: string };
      recalls.push({ path, body });
      if (control.fail) {
        control.fail = false;
        return Response.json({ error: "internal", login: "" }, { status: 500 });
      }
      kept.done.add(completion[1]);
      return Response.json({
        recall: body.next === "retire" ? "excluded" : "included",
      });
    }
    if (
      /^\/api\/jottings\/[^/]+\/recall$/.test(path) &&
      request.method === "PUT"
    ) {
      const body = (await request.json()) as { recall: string };
      recalls.push({ path, body });
      return Response.json(body);
    }
    const own = /^\/api\/jottings\/([^/]+)$/.exec(path);
    if (own && (request.method === "PUT" || request.method === "DELETE")) {
      const body =
        request.method === "PUT"
          ? ((await request.json()) as Partial<ListedJotting>)
          : undefined;
      const version = new URL(request.url).searchParams.get("version");
      writes.push({
        method: request.method,
        path: version ? `${path}?version=${version}` : path,
        ...(body && { body }),
      });
      if (control.conflict || control.broken) {
        const status = control.conflict ? 409 : 500;
        control.conflict = control.broken = false;
        return Response.json(
          { error: status === 409 ? "jotting_changed" : "persistence_failed" },
          { status },
        );
      }
      if (!body) {
        listed = listed.filter((item) => item.id !== own[1]);
        return new Response(null, { status: 204 });
      }
      listed = listed.map((item) =>
        item.id === own[1]
          ? { ...item, ...body, version: "2", pending: true }
          : item,
      );
      return Response.json({ version: "2" });
    }
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
      return Response.json({
        date: "2026-10-20",
        jottings: pile.map((card) => ({
          ...card,
          done: kept.done.has(card.id),
        })),
      });
    throw new Error(`unexpected request ${path}`);
  });
  return { queries, uploads, recalls, writes, control, kept };
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

// specs/test-cases/web/echo/echo-page.md#only-completing-a-recall-card-may-take-it-out-of-the-pile
// specs/test-cases/web/echo/echo-page.md#a-zoomed-jotting-must-say-whether-it-takes-part-in-recall
it("takes a recall card out of the pile only when it is completed, and remembers it", async () => {
  const { recalls, control, kept } = serve();
  const stored = vi.spyOn(Storage.prototype, "setItem");
  const { router, user } = mountApp("/jottings");
  await screen.findByText("回响第1张");
  expect(recallCards()).toEqual(["r1", "r2", "r3", "r4", "r5"]);
  const card = (id: string) =>
    within(recall())
      .getAllByRole("button")
      .find((element) => element.dataset.id === id)!;
  const dialog = () => screen.getByRole("dialog").parentElement!;

  // The click that puts the spread pile away does not also filter by the tag under it.
  await user.click(card("r1"));
  expect(recall()).toHaveClass("spread");
  await user.click(screen.getByRole("button", { name: "#rust" }));
  expect(recall()).not.toHaveClass("spread");
  expect(address(router)).toBe("/jottings");

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
  await user.click(screen.getByRole("button", { name: "清除" }));
  await waitFor(() => expect(address(router)).toBe("/jottings"));
  expect(drawer()).not.toHaveAttribute("inert");

  // Closing a card any other way puts it back, and tells the server nothing.
  await user.click(card("r1"));
  await user.click(card("r2"));
  expect(
    within(dialog())
      .getAllByRole("button")
      .map((button) => button.textContent),
  ).toEqual(["温故", "束阁", "修订", "削简"]);
  await user.keyboard("{Escape}");
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  await user.click(card("r2"));
  await user.click(document.querySelector(".zoom-backdrop")!);
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(recallCards()).toEqual(["r1", "r2", "r3", "r4", "r5"]);
  expect(recalls).toEqual([]);
  await user.keyboard("{Escape}");
  expect(recall()).not.toHaveClass("spread");

  // 温故 and 束阁 each take their card out.
  await user.click(card("r1"));
  await user.click(card("r2"));
  await user.click(within(dialog()).getByRole("button", { name: "温故" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(recallCards()).toEqual(["r1", "r3", "r4", "r5"]);
  await user.click(card("r4"));
  await user.click(within(dialog()).getByRole("button", { name: "束阁" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(recallCards()).toEqual(["r1", "r3", "r5"]);
  expect(recalls).toEqual([
    { path: "/api/jottings/recall/2026-10-20/r2/done", body: { next: "keep" } },
    {
      path: "/api/jottings/recall/2026-10-20/r4/done",
      body: { next: "retire" },
    },
  ]);

  // A completion that fails leaves the card open, ready to try again.
  control.fail = true;
  await user.click(card("r1"));
  await user.click(within(dialog()).getByRole("button", { name: "温故" }));
  await waitFor(() =>
    expect(
      within(dialog()).getByRole("button", { name: "温故" }),
    ).toBeEnabled(),
  );
  expect(screen.getByRole("dialog")).toBeInTheDocument();
  await user.click(within(dialog()).getByRole("button", { name: "温故" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  for (const id of ["r3", "r5"]) {
    await user.click(card(id));
    await user.click(within(dialog()).getByRole("button", { name: "温故" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  }
  expect(screen.getByText("今天的回响都已看过")).toBeInTheDocument();

  // The server keeps what was completed: coming back shows no card again.
  // 时刻 asks for its day; it has nothing on it.
  vi.spyOn(globalThis, "fetch").mockImplementation(
    async (url) => bareDay(requestPath(url)) ?? Response.json([]),
  );
  await act(() => router.navigate("/"));
  serve(wall, kept);
  await act(() => router.navigate("/jottings"));
  expect(await screen.findByText("今天的回响都已看过")).toBeInTheDocument();
  expect(stored).not.toHaveBeenCalled();
});

// specs/test-cases/web/echo/echo-page.md#a-wall-jotting-must-open-in-full-by-its-time-and-come-back-as-it-was
// specs/test-cases/web/echo/echo-page.md#only-a-jotting-read-in-full-may-load-pictures-and-only-palace-media
it("reads a wall jotting in full from its time, with its Palace pictures, and hands it back", async () => {
  const recent = Date.now() - 3 * 3_600_000;
  const media = "/api/media/01a169c8-2109-774d-a490-e8537c284595";
  const { recalls } = serve([
    jotting(
      "p1",
      `硬币动画\n\n![硬币](${media})\n\n![外站](https://example.com/a.png)`,
      ["跑步"],
      recent,
    ),
    ...wall,
  ]);
  const { router, user } = mountApp("/jottings?tag=跑步");
  const card = (await screen.findByText("硬币动画")).closest("article")!;

  // On the wall pictures stay placeholders and the time has no tooltip.
  expect(card.querySelector("img")).toBeNull();
  expect(within(card).getByText("[图片：硬币]")).toBeInTheDocument();
  expect(within(card).getByText("3 小时前")).not.toHaveAttribute("data-tip");

  // The words are not a way in; the time is.
  await user.click(within(card).getByText("硬币动画"));
  expect(screen.queryByRole("dialog")).toBeNull();
  await user.click(within(card).getByRole("button", { name: "放大" }));
  const dialog = await screen.findByRole("dialog");
  expect(within(dialog).getByRole("img", { name: "硬币" })).toHaveAttribute(
    "src",
    media,
  );
  expect(within(dialog).getByText("[图片：外站]")).toBeInTheDocument();
  expect(within(dialog).getByText(absoluteTime(recent))).toBeInTheDocument();
  expect(card).toHaveStyle({ visibility: "hidden" });

  // A wall card says whether it takes part in recall, and stays open when that changes.
  expect(
    within(dialog.parentElement!.querySelector(".zoom-actions")!)
      .getAllByRole("button")
      .map((button) => button.textContent),
  ).toEqual(["修订", "束阁", "削简"]);
  await user.click(
    within(dialog.parentElement!).getByRole("button", { name: "束阁" }),
  );
  expect(
    await within(dialog.parentElement!).findByRole("button", {
      name: "启封",
    }),
  ).toBeInTheDocument();
  expect(recalls).toEqual([
    { path: "/api/jottings/p1/recall", body: { recall: "excluded" } },
  ]);
  expect(screen.getByRole("dialog")).toBe(dialog);

  // Escape closes the card and nothing else: the filter stays, the card is back.
  await user.keyboard("{Escape}");
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(address(router)).toBe("/jottings?tag=跑步");
  expect(card).toBeVisible();
  expect(card.querySelector("img")).toBeNull();
});

// specs/test-cases/web/echo/echo-page.md#a-wall-jotting-must-open-in-full-by-its-time-and-come-back-as-it-was
it("opens a body in place up to forty lines and reads a longer one in full", async () => {
  serve([
    jotting("long", `很长的札记\n\n${"一行\n\n".repeat(60)}`, [], at(21)),
    jotting("mid", `中等的札记\n\n${"一行\n\n".repeat(20)}`, [], at(20)),
  ]);
  // jsdom has no layout: a body is as tall as its paragraphs, at 20px a line, folded at eight.
  vi.spyOn(HTMLElement.prototype, "scrollHeight", "get").mockImplementation(
    function (this: HTMLElement) {
      return this.querySelectorAll("p").length * 20;
    },
  );
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(160);
  const computed = window.getComputedStyle.bind(window);
  vi.spyOn(window, "getComputedStyle").mockImplementation((element) => {
    const style = computed(element);
    return new Proxy(style, {
      get: (target, key) => {
        if (key === "lineHeight") return "20px";
        const value = Reflect.get(target, key) as unknown;
        return typeof value === "function" ? value.bind(target) : value;
      },
    });
  });
  const { user } = mountApp("/jottings");
  const mid = (await screen.findByText("中等的札记")).closest("article")!;
  const long = screen.getByText("很长的札记").closest("article")!;

  await user.click(within(mid).getByRole("button", { name: "展开" }));
  expect(within(mid).getByRole("button", { name: "收起" })).toHaveAttribute(
    "aria-expanded",
    "true",
  );
  expect(screen.queryByRole("dialog")).toBeNull();

  await user.click(within(long).getByRole("button", { name: "展开" }));
  expect(
    within(await screen.findByRole("dialog")).getByText("很长的札记"),
  ).toBeInTheDocument();
  expect(long.querySelector(".jot-body")).toHaveClass("folded");
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

// specs/test-cases/web/echo/echo-page.md#editing-a-zoomed-jotting-must-never-lose-the-draft
it("edits a zoomed jotting in place with its tags written back, and never drops a changed draft", async () => {
  const { writes, control } = serve();
  const { user } = mountApp("/jottings");
  const card = (await screen.findByText("Arc 写多了要警惕")).closest(
    "article",
  )!;
  await user.click(within(card).getByRole("button", { name: "放大" }));
  const dialog = await screen.findByRole("dialog");
  const slips = () =>
    within(dialog.parentElement!.querySelector(".zoom-actions")!);

  // The tags come back as the body's last line; an unchanged edit leaves on Escape.
  await user.click(slips().getByRole("button", { name: "修订" }));
  const title = within(dialog).getByRole("textbox", { name: "标题" });
  const body = within(dialog).getByRole("textbox", { name: "正文" });
  expect(title).toHaveValue("所有权");
  expect(body).toHaveValue("Arc 写多了要警惕\n\n#rust #[[Vibe Coding]]");
  expect(
    slips()
      .getAllByRole("button")
      .map((button) => button.textContent),
  ).toEqual(["保存", "取消"]);
  await user.keyboard("{Escape}");
  expect(within(dialog).queryByRole("textbox")).toBeNull();
  expect(screen.getByRole("dialog")).toBe(dialog);

  // A changed draft stays through Escape and a click on the backdrop.
  await user.click(slips().getByRole("button", { name: "修订" }));
  await user.keyboard("，再想想");
  await user.clear(within(dialog).getByRole("textbox", { name: "标题" }));
  await user.type(
    within(dialog).getByRole("textbox", { name: "标题" }),
    "所有",
  );
  await user.keyboard("{Escape}");
  await user.click(document.querySelector(".zoom-backdrop")!);
  expect(within(dialog).getByRole("textbox", { name: "正文" })).toHaveValue(
    "Arc 写多了要警惕，再想想\n\n#rust #[[Vibe Coding]]",
  );
  expect(writes).toEqual([]);

  // Saving takes the tags out again, names the version read, and shows on the wall at once.
  await user.click(within(dialog).getByRole("textbox", { name: "正文" }));
  await user.keyboard("{Control>}{Enter}{/Control}");
  expect(
    await within(dialog).findByRole("heading", { name: "所有" }),
  ).toBeInTheDocument();
  expect(writes).toEqual([
    {
      method: "PUT",
      path: "/api/jottings/w2",
      body: {
        version: "1",
        title: "所有",
        tags: ["rust", "Vibe Coding"],
        body: "Arc 写多了要警惕，再想想",
      },
    },
  ]);
  expect(
    within(card).getByText("Arc 写多了要警惕，再想想"),
  ).toBeInTheDocument();
  expect(within(card).getByLabelText("待整理")).toBeInTheDocument();

  // Saving unchanged asks nothing; a conflict keeps the draft and saving stays off.
  await user.click(slips().getByRole("button", { name: "修订" }));
  await user.click(slips().getByRole("button", { name: "保存" }));
  expect(within(dialog).queryByRole("textbox")).toBeNull();
  expect(writes).toHaveLength(1);
  await user.click(slips().getByRole("button", { name: "修订" }));
  await user.keyboard("又改");
  control.conflict = true;
  await user.click(slips().getByRole("button", { name: "保存" }));
  expect(await slips().findByRole("alert")).toHaveTextContent(
    "札记已在别处改过，复制修改后取消再修订",
  );
  expect(slips().getByRole("button", { name: "保存" })).toBeDisabled();
  expect(within(dialog).getByRole("textbox", { name: "正文" })).toHaveValue(
    "Arc 写多了要警惕，再想想又改\n\n#rust #[[Vibe Coding]]",
  );
  expect(writes).toHaveLength(2);

  // Any other failure keeps the draft too, and saving may be tried again.
  await user.click(slips().getByRole("button", { name: "取消" }));
  await user.click(slips().getByRole("button", { name: "修订" }));
  await user.keyboard("！");
  control.broken = true;
  await user.click(slips().getByRole("button", { name: "保存" }));
  expect(await slips().findByRole("alert")).toHaveTextContent("保存失败");
  expect(slips().getByRole("button", { name: "保存" })).toBeEnabled();
  await user.click(slips().getByRole("button", { name: "保存" }));
  await waitFor(() => expect(within(dialog).queryByRole("textbox")).toBeNull());
  expect(writes).toHaveLength(4);
});

// specs/test-cases/web/echo/echo-page.md#deleting-a-jotting-must-take-two-clicks-on-the-same-slip
it("deletes a jotting only on a second click on the same slip, from the wall and from the pile", async () => {
  const { writes, control } = serve();
  const { user } = mountApp("/jottings");
  const card = (await screen.findByText("坂本龙一的 Aqua")).closest("article")!;
  await user.click(within(card).getByRole("button", { name: "放大" }));
  const dialog = await screen.findByRole("dialog");
  const slip = () =>
    within(dialog.parentElement!.querySelector(".zoom-actions")!).getByRole(
      "button",
      { name: /^(削简|确认)$/ },
    );

  // A first click only arms the slip; three seconds after it, or losing focus, disarms it. The
  // pointer leaving neither disarms it nor starts the wait over.
  await user.click(slip());
  expect(slip()).toHaveTextContent("确认");
  expect(slip()).toHaveClass("confirming");
  fireEvent.blur(slip());
  expect(slip()).toHaveTextContent("削简");
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
  fireEvent.click(slip());
  act(() => vi.advanceTimersByTime(2999));
  expect(slip()).toHaveTextContent("确认");
  act(() => vi.advanceTimersByTime(1));
  expect(slip()).toHaveTextContent("削简");
  fireEvent.click(slip());
  act(() => vi.advanceTimersByTime(2000));
  fireEvent.mouseLeave(slip());
  expect(slip()).toHaveTextContent("确认");
  act(() => vi.advanceTimersByTime(999));
  expect(slip()).toHaveTextContent("确认");
  act(() => vi.advanceTimersByTime(1));
  expect(slip()).toHaveTextContent("削简");
  vi.useRealTimers();
  expect(writes).toEqual([]);

  // A failed deletion keeps the card open and says so.
  control.broken = true;
  await user.click(slip());
  await user.click(slip());
  expect(
    await within(dialog.parentElement!).findByRole("alert"),
  ).toHaveTextContent("删除失败");
  expect(screen.getByRole("dialog")).toBe(dialog);

  // Two clicks delete it, naming the version read; it leaves the wall.
  await user.click(slip());
  await user.click(slip());
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(writes.at(-1)).toEqual({
    method: "DELETE",
    path: "/api/jottings/w3?version=1",
  });
  expect(screen.queryByText("坂本龙一的 Aqua")).toBeNull();
  expect(screen.getByText("夜跑后的一点念头")).toBeInTheDocument();

  // From the pile, a deleted card leaves the pile as a completed one does.
  const pileCard = (id: string) =>
    within(recall())
      .getAllByRole("button")
      .find((element) => element.dataset.id === id)!;
  await user.click(pileCard("r1"));
  await user.click(pileCard("r1"));
  const opened = (await screen.findByRole("dialog")).parentElement!;
  await user.click(within(opened).getByRole("button", { name: "削简" }));
  await user.click(within(opened).getByRole("button", { name: "确认" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(writes.at(-1)).toEqual({
    method: "DELETE",
    path: "/api/jottings/r1?version=1",
  });
  expect(recallCards()).toEqual(["r2", "r3", "r4", "r5"]);
});
