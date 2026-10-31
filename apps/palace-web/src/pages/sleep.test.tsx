import { act, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeAll, beforeEach, expect, it, vi } from "vitest";
import { requestPath, testRequest } from "@/lib/test-request";
import type { Night } from "@/lib/sleep";
import { address, mountApp } from "@/test-app";

// Local times on 2026-10-29 and the days around it.
const at = (day: number, hours: number, minutes = 0) =>
  new Date(2026, 9, day, hours, minutes).getTime();

// The day menu preloads its dialogs when approached; compiling them once here keeps that cost
// out of whichever test approaches the menu first.
beforeAll(async () => {
  await Promise.all([
    import("@/components/import-dialog"),
    import("@/components/key-manager"),
  ]);
});
beforeEach(() => {
  // Only the clock is faked; timers stay real so the dial ticks and user events run as usual.
  vi.useFakeTimers({ toFake: ["Date"], shouldAdvanceTime: true });
});
afterEach(() => {
  vi.useRealTimers();
});

// An in-memory stand-in for the server's sleep: one running start and the stored nights, with
// every write recorded as "METHOD path body".
function serveSleep(state: { running: number | null; nights: Night[] }) {
  const writes: string[] = [];
  let next = 0;
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
    const request = testRequest(input);
    const path = requestPath(input);
    const query = new URL(request.url).searchParams;
    const [start, end] = [Number(query.get("start")), Number(query.get("end"))];
    const body = request.method === "GET" ? "" : await request.text();
    if (request.method !== "GET")
      writes.push(`${request.method} ${path}${body ? ` ${body}` : ""}`);
    const waking = () =>
      state.nights.filter((n) => n.ended_at >= start && n.ended_at < end);
    if (path === "/api/habits")
      return Response.json({ habits: [], checks: [] });
    if (path === "/api/timeline")
      return Response.json(
        waking().map((n) => ({
          kind: "sleep",
          id: n.id,
          occurred_at: n.ended_at,
          started_at: n.started_at,
        })),
      );
    if (path === "/api/timeline/outline")
      return Response.json(waking().map((n) => ({ kind: "sleep", id: n.id })));
    if (path === "/api/sleep/running") {
      if (request.method === "PUT") state.running = Date.now();
      return Response.json({ started_at: state.running });
    }
    if (path === "/api/sleep/running/end") {
      const night = {
        id: `ended-${next++}`,
        started_at: state.running!,
        ended_at: Date.now(),
      };
      state.running = null;
      state.nights.push(night);
      return Response.json(night);
    }
    if (path === "/api/sleep/nights") {
      if (request.method === "GET") return Response.json(waking());
      const night = { id: `recorded-${next++}`, ...JSON.parse(body) };
      state.nights.push(night);
      return Response.json(night, { status: 201 });
    }
    const id = path.split("/").at(-1);
    state.nights = state.nights.filter((n) => n.id !== id);
    if (request.method === "DELETE") return new Response(null, { status: 204 });
    const night = { id, ...JSON.parse(body) };
    state.nights.push(night);
    return Response.json(night);
  });
  return { writes };
}

// What the day menu offers besides sleep, always in this order.
const REST = ["记录工夫", "编排日课", "日课与工夫", "导入对话", "密钥管理"];

// Radix opens menus on real pointer events, which jsdom cannot produce; the keyboard path is the
// same menu. Focusing the "···" is also what reads the clock for the sleep action.
async function menuItems(user: ReturnType<typeof mountApp>["user"]) {
  (await screen.findByRole("button", { name: "当天操作" })).focus();
  await user.keyboard("{Enter}");
  const items = await screen.findAllByRole("menuitem");
  return items.map((item) => item.textContent);
}

// Core test cases:
// - `specs/test-cases/web/sleep/sleep.md#the-menu-must-offer-sleep-by-the-readers-clock`
// - `specs/test-cases/web/sleep/sleep.md#the-running-dial-must-tick-without-rebuilding-its-stop-button`
it("starts in the evening and ticks on a pinned dial whose stop button stays put", async () => {
  vi.setSystemTime(at(29, 21, 30));
  const { writes } = serveSleep({ running: null, nights: [] });
  const { user } = mountApp("/?date=2026-10-29");
  expect(await menuItems(user)).toEqual(["开始睡眠", ...REST]);
  await user.click(screen.getByRole("menuitem", { name: "开始睡眠" }));
  const stop = await screen.findByRole("button", { name: "结束睡眠" });
  const figures = screen.getByLabelText("已睡");
  const first = figures.textContent;
  // A minute on, the next tick must show other figures.
  act(() => vi.setSystemTime(Date.now() + 60_000));
  await waitFor(() => expect(figures.textContent).not.toBe(first));
  // The figures moved; the button is the same element, so its breathing never restarted.
  expect(screen.getByRole("button", { name: "结束睡眠" })).toBe(stop);
  // A running sleep ends from its dial, never from the menu.
  expect(await menuItems(user)).toEqual([...REST]);
  expect(writes).toEqual(["PUT /api/sleep/running"]);
});

// Core test case: `specs/test-cases/web/sleep/sleep.md#ending-before-six-must-ask-once-more`
it("asks before ending in the night, keeps sleeping by default, and ends when told", async () => {
  vi.setSystemTime(at(29, 23, 0));
  const { writes } = serveSleep({ running: at(29, 21, 30), nights: [] });
  const { user } = mountApp("/?date=2026-10-29");
  await user.click(await screen.findByRole("button", { name: "结束睡眠" }));
  const question = await screen.findByRole("alertdialog");
  expect(question).toHaveTextContent("现在 23:00，才睡了 1 小时 30 分。");
  expect(
    within(question).getByRole("button", { name: "继续睡" }),
  ).toHaveFocus();
  await user.click(within(question).getByRole("button", { name: "继续睡" }));
  expect(writes).toEqual([]);

  await user.click(screen.getByRole("button", { name: "结束睡眠" }));
  await user.click(
    within(await screen.findByRole("alertdialog")).getByRole("button", {
      name: "结束",
    }),
  );
  const night = await screen.findByRole("link", { name: /^睡眠 1 小时 30 分/ });
  expect(night).toHaveAttribute("href", "/sleep?date=2026-10-29");
  expect(writes).toEqual([
    `POST /api/sleep/running/end {"started_at":${at(29, 21, 30)}}`,
  ]);
});

// Core test case: `specs/test-cases/web/sleep/sleep.md#ending-before-six-must-ask-once-more`
it("ends at once in the morning", async () => {
  vi.setSystemTime(at(29, 7, 12));
  const { writes } = serveSleep({ running: at(28, 23, 46), nights: [] });
  const { user } = mountApp("/?date=2026-10-29");
  await user.click(await screen.findByRole("button", { name: "结束睡眠" }));
  expect(
    await screen.findByRole("link", {
      name: "睡眠 7 小时 26 分，23:46 入睡，07:12 醒来",
    }),
  ).toBeInTheDocument();
  expect(screen.queryByRole("alertdialog")).toBeNull();
  expect(writes).toEqual([
    `POST /api/sleep/running/end {"started_at":${at(28, 23, 46)}}`,
  ]);
});

// Core test cases:
// - `specs/test-cases/web/sleep/sleep.md#the-menu-must-offer-sleep-by-the-readers-clock`
// - `specs/test-cases/web/sleep/sleep.md#a-typed-night-must-land-on-its-wake-day`
it("records a night never started, refusing times that do not add up", async () => {
  vi.setSystemTime(at(29, 9, 0));
  const { writes } = serveSleep({ running: null, nights: [] });
  const { user } = mountApp("/?date=2026-10-29");
  expect(await menuItems(user)).toEqual(["记录睡眠", ...REST]);
  await user.click(screen.getByRole("menuitem", { name: "记录睡眠" }));
  const dialog = await screen.findByRole("dialog", { name: "记录睡眠" });
  const wake = within(dialog).getByLabelText("醒来");
  expect(
    [within(dialog).getByLabelText("入睡"), wake].map(
      (field) => (field as HTMLInputElement).value,
    ),
  ).toEqual(["23:30", "07:00"]);
  await user.clear(wake);
  await user.type(wake, "10:30");
  expect(dialog).toHaveTextContent("醒来时间还没到");
  expect(within(dialog).getByRole("button", { name: "保存" })).toBeDisabled();
  await user.clear(wake);
  await user.type(wake, "7:10");
  expect(dialog).toHaveTextContent("7 小时 40 分");
  await user.click(within(dialog).getByRole("button", { name: "保存" }));
  expect(
    await screen.findByRole("link", { name: /^睡眠 7 小时 40 分/ }),
  ).toBeInTheDocument();
  expect(writes).toEqual([
    `POST /api/sleep/nights {"started_at":${at(28, 23, 30)},"ended_at":${at(29, 7, 10)}}`,
  ]);
  // A night is recorded for today now, so the morning offers nothing more.
  expect(await menuItems(user)).toEqual([...REST]);
});

// Core test cases:
// - `specs/test-cases/web/sleep/sleep.md#a-forgotten-start-must-lapse-at-three-the-next-afternoon`
// - `specs/test-cases/web/sleep/sleep.md#the-menu-must-offer-sleep-by-the-readers-clock`
it("lets a forgotten start lapse at 15:00 and starts over in the evening", async () => {
  vi.setSystemTime(at(29, 16, 0));
  const { writes } = serveSleep({ running: at(28, 23, 46), nights: [] });
  const { user } = mountApp("/?date=2026-10-29");
  expect(await menuItems(user)).toEqual([...REST]);
  expect(screen.queryByRole("button", { name: "结束睡眠" })).toBeNull();
  await user.keyboard("{Escape}");
  act(() => vi.setSystemTime(at(29, 21, 30)));
  expect(await menuItems(user)).toEqual(["开始睡眠", ...REST]);
  await user.click(screen.getByRole("menuitem", { name: "开始睡眠" }));
  await screen.findByRole("button", { name: "结束睡眠" });
  expect(writes).toEqual(["PUT /api/sleep/running"]);
});

// Core test case: `specs/test-cases/web/sleep/sleep.md#a-night-must-be-pinned-to-its-wake-day-and-lead-to-its-history`
it("pins an earlier day's night above its cards, offers nothing for it, and leads to the history", async () => {
  vi.setSystemTime(at(29, 21, 30));
  serveSleep({
    running: null,
    nights: [
      { id: "n26", started_at: at(25, 23, 0), ended_at: at(26, 7, 0) },
      { id: "n27", started_at: at(26, 23, 40), ended_at: at(27, 6, 50) },
    ],
  });
  const { router, user } = mountApp("/?date=2026-10-27");
  const dial = await screen.findByRole("link", {
    name: "睡眠 7 小时 10 分，23:40 入睡，06:50 醒来",
  });
  // The night is not drawn again in the list, and the day is not called empty.
  expect(document.querySelectorAll("ol.timeline > li")).toHaveLength(0);
  expect(screen.queryByText("这一天还没有记录。")).toBeNull();
  // Two of the week's seven days have a night; the rest are short strokes.
  expect(
    [...dial.querySelectorAll(".week i")].map((bar) => bar.className),
  ).toEqual(["gap", "gap", "gap", "gap", "gap", "", "this"]);
  expect(await menuItems(user)).toEqual([...REST]);
  await user.keyboard("{Escape}");
  await user.click(dial);
  expect(address(router)).toBe("/sleep?date=2026-10-27");
  expect(
    await screen.findByRole("link", { name: "← 10 月 27 日" }),
  ).toHaveAttribute("href", "/?date=2026-10-27");
});

// Core test case: `specs/test-cases/web/sleep/sleep.md#correcting-or-deleting-a-night-must-move-or-remove-its-moment`
it("corrects, deletes and fills in nights from the history", async () => {
  vi.setSystemTime(at(29, 21, 30));
  const { writes } = serveSleep({
    running: null,
    nights: [
      { id: "n27", started_at: at(26, 23, 40), ended_at: at(27, 6, 50) },
      { id: "n29", started_at: at(28, 23, 46), ended_at: at(29, 7, 12) },
    ],
  });
  const { user } = mountApp("/sleep?date=2026-10-29");
  const rows = async () =>
    within(await screen.findByRole("list")).getAllByRole("button");
  expect((await rows()).map((row) => row.textContent)).toEqual([
    "10/29 周四7:26",
    "10/28 周三无",
    "10/27 周二7:10",
  ]);
  expect(screen.getByText("7 小时 18 分")).toBeInTheDocument();

  await user.click((await rows())[2]);
  let dialog = await screen.findByRole("dialog", { name: "10 月 27 日 周二" });
  const bed = within(dialog).getByLabelText("入睡");
  await user.clear(bed);
  await user.type(bed, "23:10");
  await user.click(within(dialog).getByRole("button", { name: "保存" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());

  await user.click((await rows())[0]);
  dialog = await screen.findByRole("dialog", { name: "10 月 29 日 周四" });
  await user.click(within(dialog).getByRole("button", { name: "删除这一晚" }));
  await user.click(within(dialog).getByRole("button", { name: "确认删除" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());

  await user.click((await rows())[1]);
  dialog = await screen.findByRole("dialog", { name: "补记 10 月 28 日" });
  await user.click(within(dialog).getByRole("button", { name: "保存" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());

  expect(writes).toEqual([
    `PUT /api/sleep/nights/n27 {"started_at":${at(26, 23, 10)},"ended_at":${at(27, 6, 50)}}`,
    "DELETE /api/sleep/nights/n29",
    // A new night starts from the usual times: here the one night left, 23:10 to 06:50.
    `POST /api/sleep/nights {"started_at":${at(27, 23, 10)},"ended_at":${at(28, 6, 50)}}`,
  ]);
});
