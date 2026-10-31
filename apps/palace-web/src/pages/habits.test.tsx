import { screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeAll, beforeEach, expect, it, vi } from "vitest";
import { requestPath, testRequest } from "@/lib/test-request";
import type { Activity, ActivityType } from "@/lib/activities";
import type { Habit, HabitCheck } from "@/lib/habits";
import { address, mountApp } from "@/test-app";

// Local times on 2026-10-31, a Saturday, and the days before it.
const at = (day: number, hours: number, minutes = 0) =>
  new Date(2026, 9, day, hours, minutes).getTime();

beforeAll(async () => {
  await Promise.all([
    import("@/components/import-dialog"),
    import("@/components/key-manager"),
    import("@/pages/habits"),
  ]);
});
beforeEach(() => {
  vi.useFakeTimers({ toFake: ["Date"], shouldAdvanceTime: true });
  vi.setSystemTime(at(31, 21, 30));
});
afterEach(() => {
  vi.useRealTimers();
});

type State = {
  habits: Habit[];
  checks: HabitCheck[];
  types: ActivityType[];
  activities: Activity[];
  // Makes the next write fail, as a dropped connection would.
  failNext?: boolean;
};

function habit(id: string, name: string, times: number): Habit {
  return {
    id,
    name,
    mark: name[0],
    times_per_day: times,
    created_at: at(1, 9),
    retired: false,
  };
}

// An in-memory stand-in for the server's 日课 and 工夫, recording every write as
// "METHOD path body". Sleep is empty.
function serve(state: State) {
  const writes: string[] = [];
  let next = 0;
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
    const request = testRequest(input);
    const path = requestPath(input);
    const query = new URL(request.url).searchParams;
    const body = request.method === "GET" ? "" : await request.text();
    if (request.method !== "GET") {
      writes.push(`${request.method} ${path}${body ? ` ${body}` : ""}`);
      if (state.failNext) {
        state.failNext = false;
        return Response.json({ error: "internal" }, { status: 500 });
      }
    }
    const [start, end] = [Number(query.get("start")), Number(query.get("end"))];
    const starting = () =>
      state.activities.filter(
        (a) => a.started_at >= start && a.started_at < end,
      );
    if (path === "/api/sleep/nights") return Response.json([]);
    if (path === "/api/sleep/running")
      return Response.json({ started_at: null });
    if (path === "/api/timeline/outline")
      return Response.json(
        starting().map(({ id }) => ({ kind: "activity", id })),
      );
    if (path === "/api/timeline")
      return Response.json(
        starting().map(({ started_at, ...rest }) => ({
          kind: "activity",
          occurred_at: started_at,
          ...rest,
        })),
      );
    if (path === "/api/habits" && request.method === "GET") {
      const [from, to] = [query.get("from")!, query.get("to")!];
      return Response.json({
        habits: state.habits.filter((h) => !h.retired),
        checks: state.checks.filter((c) => c.day >= from && c.day <= to),
      });
    }
    if (path === "/api/habits") {
      const input = JSON.parse(body);
      const created = {
        ...habit(`new-${next++}`, input.name, input.times_per_day),
        mark: input.mark,
      };
      state.habits.push(created);
      return Response.json(created, { status: 201 });
    }
    const check = /^\/api\/habits\/([^/]+)\/checks\/(.+)$/.exec(path);
    if (check) {
      const [, id, day] = check;
      const { count } = JSON.parse(body);
      state.checks = state.checks.filter(
        (c) => !(c.habit_id === id && c.day === day),
      );
      if (count > 0) state.checks.push({ habit_id: id, day, count });
      return Response.json({ habit_id: id, day, count });
    }
    if (path.endsWith("/retire")) {
      state.habits.find((h) => path.includes(h.id))!.retired = true;
      return new Response(null, { status: 204 });
    }
    if (path.startsWith("/api/habits/")) {
      const edited = {
        ...state.habits.find((h) => path.endsWith(h.id))!,
        ...JSON.parse(body),
      };
      state.habits = state.habits.map((h) => (h.id === edited.id ? edited : h));
      return Response.json(edited);
    }
    if (path === "/api/activity-types" && request.method === "GET")
      return Response.json(state.types);
    if (path === "/api/activity-types") {
      const added = { id: `type-${next++}`, name: JSON.parse(body).name };
      state.types.push(added);
      return Response.json(added, { status: 201 });
    }
    if (path === "/api/activities" && request.method === "GET")
      return Response.json(starting());
    const recorded = (id: string) => {
      const input = JSON.parse(body);
      const type = state.types.find((t) => t.id === input.type_id)!;
      const activity = { id, type_name: type.name, ...input };
      state.activities = [
        ...state.activities.filter((a) => a.id !== id),
        activity,
      ];
      return activity;
    };
    if (path === "/api/activities")
      return Response.json(recorded(`activity-${next++}`), { status: 201 });
    const id = path.split("/").at(-1)!;
    if (request.method === "DELETE") {
      state.activities = state.activities.filter((a) => a.id !== id);
      return new Response(null, { status: 204 });
    }
    return Response.json(recorded(id));
  });
  return { writes };
}

// Radix opens menus on real pointer events, which jsdom cannot produce; the keyboard path is the
// same menu.
async function chooseFromMenu(
  user: ReturnType<typeof mountApp>["user"],
  item: string,
) {
  (await screen.findByRole("button", { name: "当天操作" })).focus();
  await user.keyboard("{Enter}");
  await user.click(await screen.findByRole("menuitem", { name: item }));
}

// Core test case: `specs/test-cases/web/habits/habits.md#tapping-a-seal-must-cycle-the-days-count`
it("tapping a seal cycles the day's count", async () => {
  const state: State = {
    habits: [habit("workout", "锻炼", 2), habit("reading", "读书", 1)],
    checks: [],
    types: [],
    activities: [],
  };
  const { writes } = serve(state);
  const { user } = mountApp("/?date=2026-10-31");
  const seal = await screen.findByRole("button", { name: "锻炼 0/2" });
  expect(screen.getByRole("group", { name: "日课" })).toHaveTextContent("锻读");

  await user.click(seal);
  expect(screen.getByRole("button", { name: "锻炼 1/2" })).toHaveAttribute(
    "data-fill",
    "part",
  );
  await user.click(screen.getByRole("button", { name: "锻炼 1/2" }));
  expect(screen.getByRole("button", { name: "锻炼 2/2" })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await user.click(screen.getByRole("button", { name: "锻炼 2/2" }));
  await screen.findByRole("button", { name: "锻炼 0/2" });
  await waitFor(() => expect(writes).toHaveLength(3));
  expect(writes).toEqual([
    'PUT /api/habits/workout/checks/2026-10-31 {"count":1}',
    'PUT /api/habits/workout/checks/2026-10-31 {"count":2}',
    'PUT /api/habits/workout/checks/2026-10-31 {"count":0}',
  ]);

  // The day before is ticked as its own day.
  await user.keyboard("{ArrowLeft}");
  await screen.findByRole("heading", { name: /Oct 30, 2026/ });
  await user.click(await screen.findByRole("button", { name: "读书" }));
  await waitFor(() =>
    expect(writes.at(-1)).toBe(
      'PUT /api/habits/reading/checks/2026-10-30 {"count":1}',
    ),
  );

  // A failed write gives the seal back the server's count.
  state.failNext = true;
  await user.click(screen.getByRole("button", { name: "读书" }));
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "读书" })).toHaveAttribute(
      "data-fill",
      "done",
    ),
  );
});

// Core test case: `specs/test-cases/web/habits/habits.md#recording-an-activity-must-read-the-hours-the-reader-typed`
it("records an activity from the day menu", async () => {
  const state: State = {
    habits: [],
    checks: [],
    types: [
      { id: "run", name: "跑步" },
      { id: "lift", name: "力量" },
    ],
    activities: [
      {
        id: "old",
        type_id: "lift",
        type_name: "力量",
        started_at: at(30, 20),
        minutes: 50,
        note: null,
      },
    ],
  };
  const { writes } = serve(state);
  const { user } = mountApp("/?date=2026-10-28");
  await chooseFromMenu(user, "记录工夫");
  const dialog = await screen.findByRole("dialog", {
    name: "补记 10 月 28 日 的工夫",
  });
  // The most recent type comes first, with its usual length; another day starts at seven.
  const types = within(dialog).getByRole("group", { name: "类型" });
  await waitFor(() =>
    expect(within(types).getByRole("button", { name: "力量" })).toHaveAttribute(
      "aria-pressed",
      "true",
    ),
  );
  expect(within(dialog).getByLabelText("开始")).toHaveValue("07:00");
  expect(within(dialog).getByLabelText("时长（分钟）")).toHaveValue("50");

  // A new type is added in place and picked; its length starts from the default.
  await user.click(within(types).getByRole("button", { name: "新类型" }));
  await user.type(within(types).getByLabelText("新类型的名称"), "骑车{Enter}");
  await waitFor(() =>
    expect(within(types).getByRole("button", { name: "骑车" })).toHaveAttribute(
      "aria-pressed",
      "true",
    ),
  );
  expect(within(dialog).getByLabelText("时长（分钟）")).toHaveValue("30");

  await user.clear(within(dialog).getByLabelText("开始"));
  await user.type(within(dialog).getByLabelText("开始"), "07:30");
  await user.clear(within(dialog).getByLabelText("时长（分钟）"));
  await user.type(within(dialog).getByLabelText("时长（分钟）"), "1:30");
  await user.type(within(dialog).getByLabelText("备注"), "环湖");
  expect(dialog).toHaveTextContent("07:30 – 09:00 · 90 分钟");
  await user.click(within(dialog).getByRole("button", { name: "保存" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(writes).toEqual([
    'POST /api/activity-types {"name":"骑车"}',
    `POST /api/activities {"type_id":"type-0","started_at":${at(28, 7, 30)},"minutes":90,"note":"环湖"}`,
  ]);

  // The card reads its span; its title opens the correction, which can also delete it.
  const card = await screen.findByRole("button", { name: /工夫\s*骑车/ });
  expect(
    [...card.closest("li")!.querySelectorAll(".meta > span")].map(
      (span) => span.textContent,
    ),
  ).toEqual(["07:30 – 09:00", "90 分钟", "环湖"]);
  await user.click(card);
  const edit = await screen.findByRole("dialog", {
    name: "10 月 28 日 周三 · 工夫",
  });
  await user.clear(within(edit).getByLabelText("时长（分钟）"));
  await user.type(within(edit).getByLabelText("时长（分钟）"), "80");
  await user.click(within(edit).getByRole("button", { name: "保存" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(writes.at(-1)).toBe(
    `PUT /api/activities/activity-1 {"type_id":"type-0","started_at":${at(28, 7, 30)},"minutes":80,"note":"环湖"}`,
  );
  await user.click(await screen.findByRole("button", { name: /工夫\s*骑车/ }));
  const again = await screen.findByRole("dialog");
  await user.click(within(again).getByRole("button", { name: "删除" }));
  await user.click(within(again).getByRole("button", { name: "确认删除" }));
  await waitFor(() =>
    expect(screen.queryByRole("button", { name: /工夫\s*骑车/ })).toBeNull(),
  );
  expect(writes.at(-1)).toBe("DELETE /api/activities/activity-1");
});

it("refuses an activity that has not ended yet, and opens with g", async () => {
  serve({
    habits: [],
    checks: [],
    types: [{ id: "run", name: "跑步" }],
    activities: [],
  });
  const { user } = mountApp("/?date=2026-10-31");
  await screen.findByRole("heading", { name: /Oct 31, 2026/ });
  await user.keyboard("g");
  const dialog = await screen.findByRole("dialog", { name: "记录工夫" });
  // Today it defaults to just finished: 30 minutes up to 21:30.
  await waitFor(() =>
    expect(within(dialog).getByLabelText("开始")).toHaveValue("21:00"),
  );
  await user.clear(within(dialog).getByLabelText("开始"));
  await user.type(within(dialog).getByLabelText("开始"), "21:20");
  expect(dialog).toHaveTextContent("还没结束，结束后再记");
  expect(within(dialog).getByRole("button", { name: "保存" })).toBeDisabled();
});

it("arranges habits: edits, adds and retires, writing only what changed", async () => {
  const state: State = {
    habits: [
      habit("workout", "锻炼", 2),
      habit("reading", "读书", 1),
      habit("calm", "冥想", 1),
    ],
    checks: [],
    types: [],
    activities: [],
  };
  const { writes } = serve(state);
  const { user } = mountApp("/?date=2026-10-31");
  await screen.findByRole("button", { name: "锻炼 0/2" });
  await chooseFromMenu(user, "编排日课");
  const dialog = await screen.findByRole("dialog", { name: "编排日课" });
  await user.selectOptions(
    within(dialog).getByLabelText("第 1 门的每天次数"),
    "每天 3 次",
  );
  const stops = within(dialog).getAllByRole("button", { name: "停用" });
  await user.click(stops[2]);
  await user.click(within(dialog).getByRole("button", { name: "＋ 新增一门" }));
  await user.type(within(dialog).getByLabelText("第 3 门的名称"), "喝水");
  await user.selectOptions(
    within(dialog).getByLabelText("第 3 门的每天次数"),
    "每天 8 次",
  );
  await user.click(within(dialog).getByRole("button", { name: "保存" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(writes.sort()).toEqual(
    [
      "POST /api/habits/calm/retire",
      'PUT /api/habits/workout {"name":"锻炼","mark":"锻","times_per_day":3}',
      'POST /api/habits {"name":"喝水","mark":"喝","times_per_day":8}',
    ].sort(),
  );
  await screen.findByRole("button", { name: "喝水 0/8" });
  expect(screen.queryByRole("button", { name: "冥想" })).toBeNull();
});

it("ticks any day of the grid and draws each activity type as its own curve", async () => {
  const state: State = {
    habits: [habit("workout", "锻炼", 2)],
    checks: [{ habit_id: "workout", day: "2026-10-30", count: 2 }],
    types: [
      { id: "run", name: "跑步" },
      { id: "ride", name: "骑车" },
    ],
    activities: [
      {
        id: "a",
        type_id: "run",
        type_name: "跑步",
        started_at: at(30, 7),
        minutes: 35,
        note: null,
      },
      {
        id: "b",
        type_id: "ride",
        type_name: "骑车",
        started_at: at(25, 9),
        minutes: 95,
        note: null,
      },
      {
        id: "c",
        type_id: "run",
        type_name: "跑步",
        started_at: at(29, 7),
        minutes: 30,
        note: null,
      },
    ],
  };
  const { writes } = serve(state);
  const { router, user } = mountApp("/habits?date=2026-10-30");
  await user.click(
    await screen.findByRole("button", { name: "10/29 周四 锻炼 0/2" }),
  );
  await waitFor(() =>
    expect(writes).toEqual([
      'PUT /api/habits/workout/checks/2026-10-29 {"count":1}',
    ]),
  );
  expect(
    screen.getByRole("button", { name: "10/30 周五 锻炼 2/2" }),
  ).toHaveAttribute("data-fill", "done");

  // Largest total first; each row labels its own peak.
  const curves = await screen.findAllByRole("img");
  expect(curves.map((curve) => curve.getAttribute("aria-label"))).toEqual([
    "骑车近 30 天每天的时长，最多 1:35",
    "跑步近 30 天每天的时长，最多 0:35",
  ]);
  expect(screen.getByText("2 次 · 共 1:05")).toBeInTheDocument();

  await user.click(screen.getByRole("link", { name: "← 10 月 30 日" }));
  await waitFor(() => expect(address(router)).toBe("/?date=2026-10-30"));
});
