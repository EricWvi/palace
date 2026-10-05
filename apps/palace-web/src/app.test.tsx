import { act, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { address, mountApp } from "./test-app";
import type { Detail, Moment } from "./lib/api";
import { dayHeading, dayRange, parseDay, today } from "./lib/day";
import { testRequest, requestPath } from "./lib/test-request";

const day = parseDay("2025-09-30")!;
const moments: Moment[] = [
  {
    kind: "conversation",
    id: "p1",
    occurred_at: day.getTime() + 3_600_000,
    conversation_id: "c1",
    title: "计划周末出行",
    source: "chatgpt",
    message_count: 20,
    excerpt: [
      { role: "user", text: "这个周末想出去走走" },
      { role: "assistant", text: "好主意！" },
    ],
  },
];
const detail: Detail = {
  conversation: { id: "new", owner_id: "owner", source: "gemini" },
  messages: [
    {
      id: "head-new",
      role: "user",
      content:
        "**回答**\n\n<script>alert(1)</script>\n\n[危险](javascript:alert(1))",
      owner_id: "owner",
      conversation_id: "new",
      parent_message_id: null,
      created_order: 1,
    },
  ],
  paths: [
    {
      id: "path-new",
      title: "我的收藏",
      session_id: "my-session",
      head_message_id: "head-new",
      occurred_at: day.getTime(),
      created_at: 1,
      updated_at: 1,
      message_count: 1,
      original_link: "https://gemini.google.com/app/my-session",
    },
  ],
};
function mockApi() {
  return vi.spyOn(globalThis, "fetch").mockImplementation(async (url) => {
    const path = requestPath(url);
    if (path === "/api/timeline") {
      const query = new URL(testRequest(url).url).searchParams;
      return Response.json(
        Number(query.get("start")) === dayRange(day).start ? moments : [],
      );
    }
    if (path === "/api/import/file")
      return Response.json({
        import_id: "import",
        created: 1,
        reused: 0,
        conversation_id: "new",
        path_id: "path-new",
        head_message_id: "head-new",
      });
    return Response.json(detail);
  });
}
// Radix opens menus on real pointer events, which jsdom cannot produce; the keyboard path is the
// same menu, and browser tests cover the mouse.
async function openImport(user: ReturnType<typeof mountApp>["user"]) {
  (await screen.findByRole("button", { name: "当天操作" })).focus();
  await user.keyboard("{Enter}");
  await user.click(screen.getByRole("menuitem", { name: "导入对话" }));
}
const scrolled = vi.fn();
beforeEach(() => {
  // jsdom lays nothing out, so it has no scrollIntoView of its own.
  Element.prototype.scrollIntoView = scrolled;
});
afterEach(() => {
  scrolled.mockReset();
});

// Core test cases:
// - `specs/test-cases/web/navigation/text-nav-and-day-routes.md#navigation-must-mark-only-the-owning-section-and-keep-inert-items-out-of-reach`
// - `specs/test-cases/web/navigation/text-nav-and-day-routes.md#each-moment-kind-must-open-in-exactly-one-way`
// - `specs/test-cases/server/moment/moment-timeline.md#conversation-cards-must-summarize-their-own-path`
it("marks only 时刻, keeps other sections out of reach, and links cards to the reading page", async () => {
  const fetch = mockApi();
  const { user } = mountApp("/?date=2025-09-30");
  const card = (await screen.findByText("计划周末出行")).closest("li")!;
  const nav = screen.getByRole("navigation", { name: "主导航" });
  expect(within(nav).getAllByRole("link")).toEqual([
    within(nav).getByRole("link", { name: "时刻" }),
  ]);
  expect(within(nav).getByRole("link", { name: "时刻" })).toHaveAttribute(
    "aria-current",
    "page",
  );
  for (const section of ["行事", "旅途", "摘星", "回响"])
    expect(within(nav).getByText(section)).not.toHaveAttribute("aria-current");
  // Tabbing from the start reaches 时刻, then the date; inert words are skipped.
  await user.tab();
  expect(document.activeElement).toHaveTextContent("时刻");
  await user.tab();
  expect(document.activeElement).toHaveAccessibleName("Sep 30, 2025，选择日期");
  expect(within(card).getByRole("link")).toHaveAttribute(
    "href",
    "/conversations/c1?path=p1&date=2025-09-30",
  );
  expect(within(card).getByText("对话")).toHaveClass("kind");
  expect(within(card).getByText("ChatGPT")).toBeInTheDocument();
  expect(within(card).getByText("20 条消息")).toBeInTheDocument();
  expect(
    [...card.querySelectorAll("blockquote p")].map((p) => p.textContent),
  ).toEqual(["你：这个周末想出去走走", "ChatGPT：好主意！"]);
  // A conversation has no fold on the timeline; its only way in is the title link.
  expect(within(card).queryByRole("button")).toBeNull();
  const request = new URL(testRequest(fetch.mock.calls[0][0]).url);
  expect(Object.fromEntries(request.searchParams)).toEqual({
    start: String(dayRange(day).start),
    end: String(dayRange(day).end),
  });
});

// Core test case: `specs/test-cases/web/navigation/text-nav-and-day-routes.md#day-timeline-address-must-restore-the-viewed-day`
it("restores the day from the address, falls back to today, and steps days in place", async () => {
  mockApi();
  const invalid = mountApp("/?date=2025-02-30");
  expect(
    await screen.findByRole("button", { name: /选择日期/ }),
  ).toHaveTextContent(dayHeading(today()));
  expect(await screen.findByText("这一天还没有记录。")).toBeInTheDocument();
  expect(address(invalid.router)).toBe("/?date=2025-02-30");
  invalid.router.dispose();
  document.body.innerHTML = "";

  const { router, user } = mountApp("/?date=2025-09-30");
  await screen.findByText("计划周末出行");
  await user.keyboard("{ArrowRight}");
  expect(screen.getByRole("button", { name: /选择日期/ })).toHaveTextContent(
    "Oct 1, 2025",
  );
  expect([address(router), router.state.historyAction]).toEqual([
    "/?date=2025-10-01",
    "REPLACE",
  ]);
  await user.keyboard("{ArrowLeft}");
  expect(await screen.findByText("计划周末出行")).toBeInTheDocument();
  expect([address(router), router.state.historyAction]).toEqual([
    "/?date=2025-09-30",
    "REPLACE",
  ]);
});

// Core test case: `specs/test-cases/web/navigation/text-nav-and-day-routes.md#returning-from-a-conversation-must-land-on-the-same-day-without-focusing-the-moment`
it("scrolls to the moment named in the address without focusing it, then drops the target", async () => {
  mockApi();
  const { router } = mountApp("/?date=2025-09-30&moment=p1");
  await waitFor(() => expect(address(router)).toBe("/?date=2025-09-30"));
  expect(scrolled).toHaveBeenCalledWith({ block: "center" });
  expect(scrolled.mock.contexts[0]).toBe(document.getElementById("moment-p1"));
  expect(document.activeElement).toBe(document.body);
});

// Core test case: `specs/test-cases/web/navigation/text-nav-and-day-routes.md#hover-menus-must-stay-reachable-by-keyboard-and-touch`
it("opens the day menu from the keyboard and returns focus on Escape", async () => {
  mockApi();
  const { user } = mountApp("/?date=2025-09-30");
  await screen.findByText("计划周末出行");
  const trigger = screen.getByRole("button", { name: "当天操作" });
  expect(trigger).toHaveAttribute("aria-haspopup", "menu");
  trigger.focus();
  await user.keyboard("{Enter}");
  expect(trigger).toHaveAttribute("aria-expanded", "true");
  expect(
    screen.getAllByRole("menuitem").map((item) => item.textContent),
  ).toEqual(["导入对话"]);
  await user.keyboard("{Escape}");
  expect(screen.queryByRole("menu")).toBeNull();
  expect(trigger).toHaveFocus();
  expect(trigger).toHaveAttribute("aria-expanded", "false");
  // Stepping keys are for the timeline, not for an open menu.
  expect(screen.getByRole("button", { name: /选择日期/ })).toHaveTextContent(
    "Sep 30, 2025",
  );
});

// Core test cases:
// - `specs/test-cases/server/import/import-entry.md#new-imports-must-start-from-the-day-menu-with-the-viewed-day-as-default`
// - `specs/test-cases/server/import/import-entry.md#successful-import-must-open-the-reading-page-dated-to-the-submitted-occurrence`
it("imports from the day menu at the viewed day and opens the reading page dated to it", async () => {
  const fetch = mockApi();
  const { router, user } = mountApp("/?date=2025-09-30");
  await openImport(user);
  const dialog = await screen.findByRole("dialog", { name: "导入对话" });
  const now = new Date();
  expect(
    within(dialog).getByRole("button", { name: "选择对话发生日期" }),
  ).toHaveTextContent("2025 年 09 月 30 日");
  expect(within(dialog).getByLabelText("对话发生时间")).toHaveValue(
    `${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}`,
  );
  await user.selectOptions(within(dialog).getByLabelText("来源"), "gemini");
  await user.type(within(dialog).getByLabelText("Session ID"), "my-session");
  await user.type(within(dialog).getByLabelText("标题"), "我的收藏");
  const file = new File(['[{"role":"user","content":"hello"}]'], "chat.json", {
    type: "application/json",
  });
  await user.upload(within(dialog).getByLabelText("选择对话 JSON 文件"), file);
  await user.click(within(dialog).getByRole("button", { name: "导入" }));
  expect(
    await screen.findByRole("heading", { level: 1, name: "我的收藏" }),
  ).toBeInTheDocument();
  expect(address(router)).toBe(
    "/conversations/new?path=path-new&date=2025-09-30",
  );
  expect(screen.getByRole("link", { name: "← 9 月 30 日" })).toHaveAttribute(
    "href",
    "/?date=2025-09-30&moment=path-new",
  );
  // The reading page renders Markdown without running HTML or unsafe links.
  expect(screen.getByText("回答")).toHaveProperty("tagName", "STRONG");
  expect(document.querySelector("script")).toBeNull();
  expect(screen.getByText("危险")).not.toHaveAttribute(
    "href",
    expect.stringContaining("javascript:"),
  );
  const call = fetch.mock.calls.find(
    ([url]) => requestPath(url) === "/api/import/file",
  )!;
  const body = await testRequest(call[0]).clone().formData();
  expect(
    Object.fromEntries(
      [...body.entries()].filter(
        ([key]) => !["history", "idempotency_key", "occurred_at"].includes(key),
      ),
    ),
  ).toEqual({ source: "gemini", title: "我的收藏", session_id: "my-session" });
  expect(await (body.get("history") as File).text()).toBe(
    '[{"role":"user","content":"hello"}]',
  );
  const occurred = new Date(Number(body.get("occurred_at")));
  expect([occurred.toDateString(), occurred.getHours()]).toEqual([
    day.toDateString(),
    now.getHours(),
  ]);
  // Back leads to the day the conversation was filed under, onto its new card.
  await act(() => router.navigate(-1));
  expect(router.state.location.pathname).toBe("/");
  expect(router.state.location.search).toContain("date=2025-09-30");
});

it("rejects malformed JSON before sending and keeps the form for correction", async () => {
  const fetch = mockApi();
  const { user } = mountApp("/?date=2025-09-30");
  await openImport(user);
  await user.type(await screen.findByLabelText("Session ID"), "s");
  await user.type(screen.getByLabelText("标题"), "保留标题");
  await user.upload(
    screen.getByLabelText("选择对话 JSON 文件"),
    new File(["{"], "bad.json", { type: "application/json" }),
  );
  await user.click(screen.getByRole("button", { name: "导入" }));
  expect(await screen.findByRole("alert")).toHaveTextContent(
    "文件不是有效的 JSON",
  );
  expect(screen.getByLabelText("标题")).toHaveValue("保留标题");
  expect(
    fetch.mock.calls.some(([url]) => requestPath(url) === "/api/import/file"),
  ).toBe(false);
});

it("reuses an idempotency key on retry after an ambiguous failure", async () => {
  const keys: FormDataEntryValue[] = [];
  let fail = true;
  const fetch = mockApi();
  const fallback = fetch.getMockImplementation()!;
  fetch.mockImplementation(async (url, options) => {
    if (requestPath(url) === "/api/import/file") {
      keys.push(
        (await testRequest(url).clone().formData()).get("idempotency_key")!,
      );
      if (fail) {
        fail = false;
        throw new Error("网络中断");
      }
    }
    return fallback(url, options);
  });
  const { user } = mountApp("/?date=2025-09-30");
  await openImport(user);
  await user.type(await screen.findByLabelText("Session ID"), "s");
  await user.type(screen.getByLabelText("标题"), "retry");
  await user.upload(
    screen.getByLabelText("选择对话 JSON 文件"),
    new File(['[{"role":"user","content":"x"}]'], "chat.json"),
  );
  await user.click(screen.getByRole("button", { name: "导入" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("网络中断");
  await user.click(screen.getByRole("button", { name: "导入" }));
  await screen.findByRole("heading", { level: 1, name: "我的收藏" });
  await waitFor(() => expect(keys).toHaveLength(2));
  expect(keys[0]).toBe(keys[1]);
});

it("shows network and authentication failures with a way forward", async () => {
  const fetch = vi
    .spyOn(globalThis, "fetch")
    .mockResolvedValueOnce(Response.json({}, { status: 503 }))
    .mockResolvedValueOnce(Response.json({}, { status: 401 }));
  const { user } = mountApp("/?date=2025-09-30");
  expect(await screen.findByRole("alert")).toHaveTextContent("服务暂时不可用");
  await user.click(screen.getByRole("button", { name: "重试" }));
  expect(
    await screen.findByRole("link", { name: "前往登录 →" }),
  ).toHaveAttribute("href", "/auth/login");
  expect(fetch).toHaveBeenCalledTimes(2);
});
