import { act, cleanup, screen, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { mountApp } from "@/test-app";
import type { Moment } from "@/lib/api";
import { dayRange, parseDay } from "@/lib/day";
import { requestPath, testRequest } from "@/lib/test-request";

const day = parseDay("2025-09-30")!;
function moment(id: string, title: string): Moment {
  return {
    kind: "conversation",
    id,
    occurred_at: day.getTime(),
    conversation_id: `c-${id}`,
    title,
    source: "chatgpt",
    message_count: 2,
    excerpt: [{ role: "user", text: title }],
  };
}
const outlineOf = (moments: Moment[]) =>
  moments.map(({ id, kind }) => ({ id, kind }));

// Holds every day request until the test answers it, so each arrival order can be staged.
function serveDay() {
  const pending = new Map<string, ((response: Response) => void)[]>();
  const asked: string[] = [];
  vi.spyOn(globalThis, "fetch").mockImplementation((url) => {
    const path = requestPath(url);
    const start = Number(
      new URL(testRequest(url).url).searchParams.get("start"),
    );
    asked.push(`${path}@${start === dayRange(day).start ? "day" : "other"}`);
    return new Promise((resolve) =>
      pending.set(path, [...(pending.get(path) ?? []), resolve]),
    );
  });
  // Answers the oldest open request to `path` and lets React settle on the result.
  async function answer(path: string, response: Response) {
    await waitFor(() => expect(pending.get(path)?.length).toBeTruthy());
    const [resolve, ...rest] = pending.get(path)!;
    pending.set(path, rest);
    await act(async () => resolve(response));
  }
  return { asked, answer };
}
const items = () => [...document.querySelectorAll("ol.timeline > li")];
const states = () =>
  items().map((item) => [
    (item as HTMLElement).dataset.id,
    (item as HTMLElement).dataset.state,
  ]);

// Core test cases:
// - `specs/test-cases/server/moment/timeline-loading.md#skeletons-must-appear-only-while-a-day-has-no-cards-to-show`
// - `specs/test-cases/server/moment/timeline-loading.md#each-skeleton-must-become-the-card-with-its-own-id`
it("asks for both at once, outlines the day, then turns each placeholder into its own card", async () => {
  const { asked, answer } = serveDay();
  mountApp("/?date=2025-09-30");
  await waitFor(() =>
    expect([...asked].sort()).toEqual([
      "/api/timeline/outline@day",
      "/api/timeline@day",
    ]),
  );
  const [walk, read, swim] = [
    moment("p1", "散步"),
    moment("p2", "读书"),
    moment("p3", "游泳"),
  ];
  await answer("/api/timeline/outline", Response.json(outlineOf([walk, read])));
  const list = await screen.findByRole("list", { hidden: true });
  expect(list).toHaveAttribute("aria-busy", "true");
  await waitFor(() =>
    expect(states()).toEqual([
      ["p1", "skeleton"],
      ["p2", "skeleton"],
    ]),
  );
  expect(items().every((item) => item.getAttribute("aria-hidden"))).toBe(true);
  expect(screen.queryAllByRole("link", { name: /散步|读书/ })).toEqual([]);
  const walking = items()[0];

  // Between the two requests p2 was deleted and p3 filed: the cards alone decide the list.
  await answer("/api/timeline", Response.json([swim, walk]));
  await waitFor(() =>
    expect(states()).toEqual([
      ["p3", "card"],
      ["p1", "card"],
    ]),
  );
  // The placeholder became its card in place: the very same element, now a card.
  expect(document.getElementById("moment-p1")).toBe(walking);
  expect(list).toHaveAttribute("aria-busy", "false");
  expect(screen.getAllByRole("link").map((link) => link.textContent)).toContain(
    "对话游泳",
  );
});

// Core test case: `specs/test-cases/server/moment/timeline-loading.md#skeletons-must-appear-only-while-a-day-has-no-cards-to-show`
it("shows the empty note as soon as the outline is empty, and still shows cards that arrive", async () => {
  const { answer } = serveDay();
  mountApp("/?date=2025-09-30");
  await answer("/api/timeline/outline", Response.json([]));
  expect(await screen.findByText("这一天还没有记录。")).toBeVisible();
  await answer("/api/timeline", Response.json([moment("p1", "散步")]));
  await waitFor(() => expect(states()).toEqual([["p1", "card"]]));
  expect(screen.queryByText("这一天还没有记录。")).toBeNull();
});

// Core test case: `specs/test-cases/server/moment/timeline-loading.md#skeletons-must-appear-only-while-a-day-has-no-cards-to-show`
it("shows cards without placeholders when the timeline wins or the outline fails", async () => {
  const first = serveDay();
  mountApp("/?date=2025-09-30");
  await first.answer("/api/timeline", Response.json([moment("p1", "散步")]));
  await first.answer(
    "/api/timeline/outline",
    Response.json(outlineOf([moment("p1", "散步")])),
  );
  await waitFor(() => expect(states()).toEqual([["p1", "card"]]));

  vi.restoreAllMocks();
  cleanup();
  const failing = serveDay();
  mountApp("/?date=2025-09-30");
  await failing.answer(
    "/api/timeline/outline",
    Response.json({}, { status: 503 }),
  );
  expect(screen.queryByRole("alert")).toBeNull();
  expect(items()).toEqual([]);
  await failing.answer("/api/timeline", Response.json([moment("p1", "散步")]));
  await waitFor(() => expect(states()).toEqual([["p1", "card"]]));
  expect(screen.queryByRole("alert")).toBeNull();
});

// Core test case: `specs/test-cases/server/moment/timeline-loading.md#skeletons-must-appear-only-while-a-day-has-no-cards-to-show`
it("drops the placeholders for the error when the timeline fails", async () => {
  const { answer } = serveDay();
  mountApp("/?date=2025-09-30");
  await answer(
    "/api/timeline/outline",
    Response.json(outlineOf([moment("p1", "散步")])),
  );
  await waitFor(() => expect(states()).toEqual([["p1", "skeleton"]]));
  await answer("/api/timeline", Response.json({}, { status: 503 }));
  expect(await screen.findByRole("alert")).toHaveTextContent("服务暂时不可用");
  expect(items()).toEqual([]);
});

// Core test case: `specs/test-cases/server/moment/timeline-loading.md#skeletons-must-appear-only-while-a-day-has-no-cards-to-show`
it("never outlines a day whose cards are already cached", async () => {
  const { asked, answer } = serveDay();
  const { user } = mountApp("/?date=2025-09-30");
  await answer("/api/timeline/outline", Response.json([]));
  await answer("/api/timeline", Response.json([moment("p1", "散步")]));
  await user.keyboard("{ArrowRight}");
  await answer("/api/timeline/outline", Response.json([]));
  await answer("/api/timeline", Response.json([]));
  await user.keyboard("{ArrowLeft}");
  // The cached cards show at once while they revalidate; no placeholder stands in for them.
  await waitFor(() => expect(states()).toEqual([["p1", "card"]]));
  await answer("/api/timeline", Response.json([moment("p1", "散步")]));
  expect(
    asked.filter((request) => request === "/api/timeline/outline@day"),
  ).toEqual(["/api/timeline/outline@day"]);
});
