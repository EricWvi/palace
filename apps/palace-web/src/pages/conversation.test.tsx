import { screen, waitFor, within } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { address, mountApp } from "@/test-app";
import type { Detail, Message } from "@/lib/api";
import type { components } from "@/lib/generated/api";
import { requestPath, testRequest } from "@/lib/test-request";

const chains = [
  ["U1", "A1", "U2", "A2"],
  ["U1", "A1", "U3", "A3", "U4", "A4"],
  ["U1", "A1", "U3", "A3", "U5", "A5"],
  ["U1", "A1"],
  ["U1", "A1"],
];
function tree(): Detail {
  const messages = new Map<string, Message>();
  for (const chain of chains)
    chain.forEach((id, index) =>
      messages.set(id, {
        id,
        content: id,
        role: id.startsWith("U") ? "user" : "assistant",
        owner_id: "owner",
        conversation_id: "tree",
        parent_message_id: chain[index - 1] ?? null,
        toc_line: `目录 ${id}`,
        created_order: index,
      }),
    );
  return {
    conversation: { owner_id: "owner", id: "tree", source: "chatgpt" },
    messages: [...messages.values()],
    paths: chains.map((chain, i) => ({
      id: `p${i + 1}`,
      title: `第 ${i + 1} 支`,
      session_id: `s${i + 1}`,
      head_message_id: chain.at(-1)!,
      message_count: chain.length,
      occurred_at: new Date(2025, 9, i + 1, 9, 30).getTime(),
      created_at: 1000,
      updated_at: [1000, 3000, 4000, 2000, 2000][i],
      original_link: `https://chatgpt.com/c/s${i + 1}`,
    })),
  };
}
// Serves one conversation that changes as the page edits and deletes it.
function setup(route: string) {
  let current = tree();
  const fetch = vi
    .spyOn(globalThis, "fetch")
    .mockImplementation(async (url) => {
      const request = testRequest(url);
      const path = requestPath(url);
      if (request.method === "PUT" && path.endsWith("/metadata")) {
        const body: components["schemas"]["PathMetadata"] = await request
          .clone()
          .json();
        const pathId = path.split("/")[5];
        current = {
          ...current,
          conversation: { ...current.conversation, source: body.source },
          paths: current.paths.map((p) =>
            p.id === pathId ? { ...p, title: body.title } : p,
          ),
        };
        return Response.json({
          conversation_id: "tree",
          path_id: pathId,
          ...body,
        });
      }
      if (request.method === "DELETE") {
        const pathId = path.split("/")[5];
        current = {
          ...current,
          paths: current.paths.filter((p) => p.id !== pathId),
        };
        return Response.json({ id: pathId ?? "tree" });
      }
      if (path.startsWith("/api/timeline")) return Response.json([]);
      if (path.endsWith("/annotations")) return Response.json({ items: [] });
      if (path === "/api/conversations")
        return Response.json({ items: [], next_cursor: null, total: 0 });
      return Response.json(current);
    });
  return { fetch, ...mountApp(route) };
}
// Radix opens menus on real pointer events, which jsdom cannot produce; the keyboard path is the
// same menu, and browser tests cover the mouse.
async function choose(user: ReturnType<typeof mountApp>["user"], item: string) {
  (await screen.findByRole("button", { name: "管理对话" })).focus();
  await user.keyboard("{Enter}");
  await user.click(screen.getByRole("menuitem", { name: item }));
}
const fork = (after: number) =>
  screen.getByRole("group", { name: `第 ${after} 条消息后的分支` });

// Core test case: `specs/test-cases/server/moment/timeline-loading.md#skeletons-must-appear-only-while-a-day-has-no-cards-to-show`
it("deleting a conversation invalidates the outlines of the days it was on", async () => {
  const { client, router, user } = setup("/conversations/tree?path=p1");
  const outline = ["timeline", "2025-09-30", "outline"];
  client.setQueryData(outline, [{ id: "p1", kind: "conversation" }]);
  expect(client.getQueryState(outline)?.isInvalidated).toBe(false);
  await choose(user, "删除对话");
  const dialog = await screen.findByRole("dialog", { name: "删除对话？" });
  await user.click(within(dialog).getByRole("button", { name: "删除对话" }));
  await waitFor(() => expect(address(router)).toBe("/conversations"));
  await waitFor(() =>
    expect(client.getQueryState(outline)?.isInvalidated).toBe(true),
  );
});

// Core test case: `specs/test-cases/server/conversation/message-tree.md#fork-selection-must-resolve-to-one-real-source-session`
it("defaults to the latest path and resets downstream forks to the latest matching continuation", async () => {
  const { user } = setup("/conversations/tree");
  await screen.findByText("A5");
  expect(screen.getByRole("link", { name: "继续对话 ↗" })).toHaveAttribute(
    "href",
    "https://chatgpt.com/c/s3",
  );
  await user.click(within(fork(4)).getByRole("button", { name: "第 2 支" }));
  expect(screen.getByText("A4")).toBeInTheDocument();
  expect(screen.queryByText("A5")).not.toBeInTheDocument();
  expect(screen.getByRole("link", { name: "继续对话 ↗" })).toHaveAttribute(
    "href",
    "https://chatgpt.com/c/s2",
  );
  await user.click(within(fork(2)).getByRole("button", { name: "第 1 支" }));
  expect(screen.getByText("A2")).toBeInTheDocument();
  expect(
    screen.queryByRole("group", { name: "第 4 条消息后的分支" }),
  ).not.toBeInTheDocument();
  await user.click(within(fork(2)).getByRole("button", { name: "第 3 支" }));
  expect(screen.getByText("A5")).toBeInTheDocument();
  expect(
    within(fork(4)).getByRole("button", { name: "第 3 支" }),
  ).toHaveAttribute("aria-current", "true");
});

// Core test case: `specs/test-cases/server/conversation/message-tree.md#fork-selection-must-resolve-to-one-real-source-session`
it("deep-links to internal endpoints and distinguishes sessions with identical message paths", async () => {
  const { user } = setup("/conversations/tree?path=p4");
  await screen.findByText("A1");
  expect(document.querySelectorAll(".message")).toHaveLength(2);
  expect(fork(2)).toHaveTextContent("此处分为 4 支：");
  expect(
    within(fork(2)).getByRole("button", { name: "第 4 支" }),
  ).toHaveAttribute("aria-current", "true");
  await user.click(within(fork(2)).getByRole("button", { name: "第 5 支" }));
  expect(document.querySelectorAll(".message")).toHaveLength(2);
  expect(screen.getByRole("link", { name: "继续对话 ↗" })).toHaveAttribute(
    "href",
    "https://chatgpt.com/c/s5",
  );
});

// Core test cases:
// - `specs/test-cases/server/conversation/reading-page.md#reading-page-header-must-describe-the-current-path`
// - `specs/test-cases/web/navigation/text-nav-and-day-routes.md#returning-from-a-conversation-must-land-on-the-same-day-without-focusing-the-moment`
it("titles, counts and links the current path, and switching paths only replaces the address", async () => {
  const { router, user } = setup("/conversations/tree?path=p2&date=2025-09-30");
  expect(
    await screen.findByRole("heading", { level: 1, name: "第 2 支" }),
  ).toBeInTheDocument();
  await waitFor(() => expect(document.title).toBe("第 2 支"));
  expect(screen.getByText("6 条消息")).toBeInTheDocument();
  const nav = screen.getByRole("navigation", { name: "主导航" });
  expect(within(nav).getByText("摘星")).toHaveAttribute("aria-current", "page");
  expect(screen.getByRole("link", { name: "← 9 月 30 日" })).toHaveAttribute(
    "href",
    "/?date=2025-09-30&moment=p2",
  );
  await user.click(within(fork(2)).getByRole("button", { name: "第 1 支" }));
  expect(
    screen.getByRole("heading", { level: 1, name: "第 1 支" }),
  ).toBeInTheDocument();
  await waitFor(() => expect(document.title).toBe("第 1 支"));
  expect(screen.getByText("4 条消息")).toBeInTheDocument();
  expect([address(router), router.state.historyAction]).toEqual([
    "/conversations/tree?path=p1&date=2025-09-30",
    "REPLACE",
  ]);
});

it("shows no way back when the reader did not come from a day", async () => {
  setup("/conversations/tree?path=p1");
  await screen.findByRole("heading", { level: 1, name: "第 1 支" });
  expect(screen.queryByRole("link", { name: /^←/ })).toBeNull();
});

// Core test case: `specs/test-cases/server/conversation/reading-page.md#conversation-management-must-live-only-in-the-reading-page-title-menu`
it("keeps conversation management in the title menu, destructive last", async () => {
  const { user } = setup("/conversations/tree?path=p1");
  (await screen.findByRole("button", { name: "管理对话" })).focus();
  await user.keyboard("{Enter}");
  expect(
    screen.getAllByRole("menuitem").map((item) => item.textContent),
  ).toEqual([
    "只看标注",
    "更新分支",
    "管理分支",
    "导出分支",
    "编辑对话",
    "删除对话",
  ]);
});

// Core test case: `specs/test-cases/server/conversation/reading-page.md#exporting-a-branch-must-write-the-import-format`
it("exports the branch being read in the format it can be imported from", async () => {
  const { user } = setup("/conversations/tree?path=p2");
  let blob: Blob | undefined;
  // jsdom has no object URLs and cannot follow a download link, so both are observed instead.
  URL.createObjectURL = (object) => {
    blob = object as Blob;
    return "blob:branch";
  };
  URL.revokeObjectURL = () => {};
  const downloads: string[][] = [];
  vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(function (
    this: HTMLAnchorElement,
  ) {
    downloads.push([this.href, this.download]);
  });
  await choose(user, "导出分支");
  expect(downloads).toEqual([["blob:branch", "第 2 支.json"]]);
  expect(JSON.parse(await blob!.text())).toEqual(
    ["U1", "A1", "U3", "A3", "U4", "A4"].map((id) => ({
      role: id.startsWith("U") ? "user" : "assistant",
      content: id,
    })),
  );
});

// Core test cases:
// - `specs/test-cases/server/conversation/reading-page.md#conversation-management-must-live-only-in-the-reading-page-title-menu`
// - `specs/test-cases/server/import/import-entry.md#successful-import-must-open-the-reading-page-dated-to-the-submitted-occurrence`
it("updates the branch being read straight from the title menu", async () => {
  const { fetch, router, user } = setup(
    "/conversations/tree?path=p2&date=2025-09-30",
  );
  const fallback = fetch.getMockImplementation()!;
  fetch.mockImplementation(async (url, options) =>
    testRequest(url).method === "PUT"
      ? Response.json({
          import_id: "i",
          conversation_id: "tree",
          path_id: "p2",
          head_message_id: "A4",
          created: 0,
          reused: 0,
        })
      : fallback(url, options),
  );
  await screen.findByRole("heading", { level: 1, name: "第 2 支" });
  await choose(user, "更新分支");
  const form = await screen.findByRole("dialog", { name: "更新分支" });
  expect(within(form).getByLabelText("标题")).toHaveValue("第 2 支");
  expect(within(form).getByLabelText("Session ID")).toHaveValue("s2");
  expect(
    within(form).getByRole("button", { name: "选择对话发生日期" }),
  ).toHaveTextContent("2025 年 10 月 02 日");
  await user.upload(
    within(form).getByLabelText("对话文件"),
    new File(['[{"role":"user","content":"U1"}]'], "update.json"),
  );
  await user.click(within(form).getByRole("button", { name: "保存更新" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  const call = fetch.mock.calls.find(
    ([url]) => testRequest(url).method === "PUT",
  )!;
  expect(requestPath(call[0])).toBe("/api/conversations/tree/paths/p2");
  // The way back now leads to the day the update was filed under.
  expect(address(router)).toBe("/conversations/tree?path=p2&date=2025-10-02");
});

// Core test case: `specs/test-cases/server/import/delta-update.md#the-new-messages-only-switch-must-default-off-in-update-mode-only`
it("uploads only new messages after the head being read, and forgets the choice on reopening", async () => {
  const { fetch, user } = setup("/conversations/tree?path=p2&date=2025-09-30");
  const fallback = fetch.getMockImplementation()!;
  fetch.mockImplementation(async (url, options) =>
    testRequest(url).method === "PUT"
      ? Response.json({ error: "path_head_moved", login: "" }, { status: 409 })
      : fallback(url, options),
  );
  await screen.findByRole("heading", { level: 1, name: "第 2 支" });
  await choose(user, "更新分支");
  let form = await screen.findByRole("dialog", { name: "更新分支" });
  const toggle = within(form).getByRole("switch", { name: "增量导入" });
  expect(toggle).not.toBeChecked();
  await user.click(toggle);
  expect(form).toHaveAccessibleDescription(
    "只上传新增消息，会接在当前第 6 条消息之后。",
  );
  const history = '[{"role":"user","content":"U6"}]';
  await user.upload(
    within(form).getByLabelText("对话文件"),
    new File([history], "delta.json"),
  );
  await user.click(within(form).getByRole("button", { name: "保存更新" }));
  expect(
    await within(form).findByText(
      "这条分支在别处有了新的更新，请刷新页面后重试。",
    ),
  ).toBeInTheDocument();
  const call = fetch.mock.calls.find(
    ([url]) => testRequest(url).method === "PUT",
  )!;
  expect(await testRequest(call[0]).clone().json()).toEqual({
    title: "第 2 支",
    history_mode: "delta",
    base_head_message_id: "A4",
    history,
    occurred_at: new Date(2025, 9, 2, 9, 30).getTime(),
    idempotency_key: expect.any(String),
  });
  // Switching back is a different request: the stale error goes away and the retry gets a new key.
  await user.click(toggle);
  expect(
    within(form).queryByText("这条分支在别处有了新的更新，请刷新页面后重试。"),
  ).toBeNull();
  await user.click(within(form).getByRole("button", { name: "保存更新" }));
  await within(form).findByText(
    "这条分支在别处有了新的更新，请刷新页面后重试。",
  );
  const [first, second] = await Promise.all(
    fetch.mock.calls
      .filter(([url]) => testRequest(url).method === "PUT")
      .map(([url]) => testRequest(url).clone().json()),
  );
  expect([second.history_mode, second.idempotency_key]).toEqual([
    "full",
    expect.not.stringMatching(first.idempotency_key),
  ]);
  await user.click(toggle);
  await user.keyboard("{Escape}");
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  await choose(user, "更新分支");
  form = await screen.findByRole("dialog", { name: "更新分支" });
  expect(
    within(form).getByRole("switch", { name: "增量导入" }),
  ).not.toBeChecked();
  expect(form).toHaveAccessibleDescription(
    "上传完整 JSON，只能在原有消息之后追加。",
  );
});

// Core test case: `specs/test-cases/server/import/delta-update.md#written-messages-must-append-as-a-delta-and-reset-on-reopening`
it("appends messages written in the dialog as a delta string, and returns to the file on reopening", async () => {
  const { fetch, user } = setup("/conversations/tree?path=p2&date=2025-09-30");
  const fallback = fetch.getMockImplementation()!;
  fetch.mockImplementation(async (url, options) =>
    testRequest(url).method === "PUT"
      ? Response.json({ error: "path_head_moved", login: "" }, { status: 409 })
      : fallback(url, options),
  );
  await screen.findByRole("heading", { level: 1, name: "第 2 支" });
  await choose(user, "更新分支");
  let form = await screen.findByRole("dialog", { name: "更新分支" });
  await user.click(within(form).getByRole("button", { name: "追加消息" }));
  expect(form).toHaveAccessibleDescription(
    "追加的消息会接在当前第 6 条消息之后。",
  );
  // Written messages are always a delta, so the switch and the file go away.
  expect(within(form).queryByRole("switch")).toBeNull();
  expect(within(form).queryByLabelText("对话文件")).toBeNull();
  expect(within(form).getByLabelText("第 1 条消息的角色")).toHaveValue("user");
  await user.click(within(form).getByRole("button", { name: "添加消息" }));
  await user.click(within(form).getByRole("button", { name: "添加消息" }));
  // Each new row starts on the other role from the row before it.
  expect(
    [1, 2, 3].map(
      (n) =>
        (
          within(form).getByLabelText(
            `第 ${n} 条消息的角色`,
          ) as HTMLSelectElement
        ).value,
    ),
  ).toEqual(["user", "assistant", "user"]);
  await user.click(
    within(form).getByRole("button", { name: "删除第 3 条消息" }),
  );
  await user.click(within(form).getByLabelText("第 1 条消息"));
  await user.paste("## 问题\n\n**为什么** \\\\(x\\\\)？");
  await user.click(within(form).getByRole("button", { name: "保存更新" }));
  expect(await within(form).findByRole("alert")).toHaveTextContent(
    "第 2 条消息还没有内容。",
  );
  await user.click(within(form).getByLabelText("第 2 条消息"));
  await user.paste("因为 \\\\(x^2\\\\)");
  await user.click(within(form).getByRole("button", { name: "保存更新" }));
  await within(form).findByText(
    "这条分支在别处有了新的更新，请刷新页面后重试。",
  );
  const call = fetch.mock.calls.find(
    ([url]) => testRequest(url).method === "PUT",
  )!;
  expect(await testRequest(call[0]).clone().json()).toEqual({
    title: "第 2 支",
    history_mode: "delta",
    base_head_message_id: "A4",
    history: JSON.stringify([
      // Only the reply has its copied formula delimiters turned into dollars.
      { role: "user", content: "## 问题\n\n**为什么** \\\\(x\\\\)？" },
      { role: "assistant", content: "因为 $x^2$" },
    ]),
    occurred_at: new Date(2025, 9, 2, 9, 30).getTime(),
    idempotency_key: expect.any(String),
  });
  // Back on the file side the switch starts off, so a later full file is never read as a delta.
  await user.click(within(form).getByRole("button", { name: "对话文件" }));
  expect(
    within(form).getByRole("switch", { name: "增量导入" }),
  ).not.toBeChecked();
  await user.click(within(form).getByRole("button", { name: "追加消息" }));
  await user.keyboard("{Escape}");
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  await choose(user, "更新分支");
  form = await screen.findByRole("dialog", { name: "更新分支" });
  expect(
    within(form).getByRole("button", { name: "对话文件", pressed: true }),
  ).toBeInTheDocument();
  expect(within(form).getByLabelText("对话文件")).toBeInTheDocument();
});

// Core test case: `specs/test-cases/server/conversation/reading-page.md#editing-must-update-the-current-path-title-and-the-conversation-source-everywhere`
it("renames only the current path and corrects the source of the whole conversation", async () => {
  const { fetch, user } = setup("/conversations/tree?path=p1");
  await choose(user, "编辑对话");
  const dialog = await screen.findByRole("dialog", { name: "编辑对话" });
  const title = within(dialog).getByLabelText("标题");
  expect(title).toHaveValue("第 1 支");
  expect(within(dialog).getByLabelText("来源")).toHaveValue("chatgpt");
  await user.clear(title);
  await user.type(title, "   ");
  await user.click(within(dialog).getByRole("button", { name: "保存" }));
  expect(await within(dialog).findByRole("alert")).toHaveTextContent(
    "请填写标题",
  );
  await user.clear(title);
  await user.type(title, "海边周末");
  await user.selectOptions(within(dialog).getByLabelText("来源"), "gemini");
  await user.click(within(dialog).getByRole("button", { name: "保存" }));
  await waitFor(() =>
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
  );
  expect(
    await screen.findByRole("heading", { level: 1, name: "海边周末" }),
  ).toBeInTheDocument();
  expect(document.querySelector(".meta")).toHaveTextContent("Gemini");
  expect(document.querySelectorAll(".who")[1]).toHaveTextContent("Gemini");
  const saved = testRequest(
    fetch.mock.calls.find(([url]) => testRequest(url).method === "PUT")![0],
  );
  expect([requestPath(saved), await saved.clone().json()]).toEqual([
    "/api/conversations/tree/paths/p1/metadata",
    { title: "海边周末", source: "gemini" },
  ]);
  // The sibling branch keeps its own title.
  await user.click(within(fork(2)).getByRole("button", { name: "第 3 支" }));
  expect(
    await screen.findByRole("heading", { level: 1, name: "第 3 支" }),
  ).toBeInTheDocument();
});

// Core test cases:
// - `specs/test-cases/server/conversation/reading-page.md#deleting-a-conversation-must-return-to-where-the-reader-came-from`
// - `specs/test-cases/web/stars/contents-page.md#returning-to-the-list-must-restore-the-search-and-drop-stale-rows`
// - `specs/test-cases/web/stars/notes.md#stars-navigation-must-open-the-note-list-and-every-kind`
it.each([
  ["/conversations/tree?path=p1&date=2025-09-30", "/?date=2025-09-30"],
  ["/conversations/tree?path=p1", "/conversations"],
])("after deleting %s, replaces the page with %s", async (route, landing) => {
  const { router, user } = setup(route);
  await choose(user, "删除对话");
  const dialog = await screen.findByRole("dialog", { name: "删除对话？" });
  await user.click(within(dialog).getByRole("button", { name: "删除对话" }));
  await waitFor(() => expect(address(router)).toBe(landing));
  expect(router.state.historyAction).toBe("REPLACE");
});

// Core test case: `specs/test-cases/server/conversation/reading-page.md#deleting-a-path-must-remove-its-moment-and-keep-reading-the-remaining-tree`
it("falls back to a remaining path after deleting the one being read, keeping the day", async () => {
  const { router, user } = setup("/conversations/tree?path=p3&date=2025-09-30");
  await screen.findByRole("heading", { level: 1, name: "第 3 支" });
  await choose(user, "管理分支");
  await user.click(await screen.findByRole("button", { name: "删除分支 s3" }));
  await user.click(screen.getByRole("button", { name: "确认删除分支" }));
  await waitFor(() =>
    expect(screen.queryByRole("button", { name: "删除分支 s3" })).toBeNull(),
  );
  await user.keyboard("{Escape}");
  expect(
    await screen.findByRole("heading", { level: 1, name: "第 2 支" }),
  ).toBeInTheDocument();
  expect([address(router), router.state.historyAction]).toEqual([
    "/conversations/tree?path=p2&date=2025-09-30",
    "REPLACE",
  ]);
  expect(
    screen.getByRole("link", { name: "← 9 月 30 日" }),
  ).toBeInTheDocument();
});

// Core test case: `specs/test-cases/server/import/import-entry.md#successful-import-must-open-the-reading-page-dated-to-the-submitted-occurrence`
it("switches to an imported branch and dates the way back to its occurrence", async () => {
  const { fetch, router, user } = setup(
    "/conversations/tree?path=p1&date=2025-09-30",
  );
  const fallback = fetch.getMockImplementation()!;
  fetch.mockImplementation(async (url, options) =>
    testRequest(url).method === "POST"
      ? Response.json({
          import_id: "i",
          conversation_id: "tree",
          path_id: "p5",
          head_message_id: "A1",
          created: 0,
          reused: 2,
        })
      : fallback(url, options),
  );
  await choose(user, "管理分支");
  await user.click(await screen.findByRole("button", { name: "新建分支" }));
  const form = await screen.findByRole("dialog", { name: "新建分支" });
  // A branch starts with the title of the path being read, and may be renamed.
  expect(within(form).getByLabelText("标题")).toHaveValue("第 1 支");
  await user.type(within(form).getByLabelText("Session ID"), "s9");
  await user.upload(
    within(form).getByLabelText("对话文件"),
    new File(['[{"role":"user","content":"U1"}]'], "branch.json"),
  );
  await user.click(within(form).getByRole("button", { name: "导入分支" }));
  await waitFor(() => expect(router.state.location.search).toContain("p5"));
  const today = new Date();
  const day = `${today.getFullYear()}-${String(today.getMonth() + 1).padStart(2, "0")}-${String(today.getDate()).padStart(2, "0")}`;
  expect(address(router)).toBe(`/conversations/tree?path=p5&date=${day}`);
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
});

// Core test case: `specs/test-cases/server/conversation/reading-page.md#the-message-toc-must-list-every-message-of-the-current-path-by-its-opening`
it("lists the read path's messages by their server lines, and a jump keeps its mark until the reader scrolls", async () => {
  const scrolled = vi.fn();
  // jsdom lays nothing out, so it has no scrollIntoView of its own.
  Element.prototype.scrollIntoView = function (this: Element) {
    scrolled(this.id);
  };
  try {
    const { user } = setup("/conversations/tree?path=p3");
    const toc = await screen.findByRole("navigation", { name: "消息目录" });
    const lines = () =>
      within(toc)
        .getAllByRole("link")
        .map((link) => link.textContent);
    expect(lines()).toEqual(
      ["U1", "A1", "U3", "A3", "U5", "A5"].map((id) => `目录 ${id}`),
    );
    const current = () =>
      within(toc)
        .getAllByRole("link")
        .findIndex((link) => link.getAttribute("aria-current") === "true");
    // Nothing is laid out, so every message has passed the reading line and the last one is read.
    await waitFor(() => expect(current()).toBe(5));

    const target = within(toc).getByRole("link", { name: "目录 U3" });
    await user.click(target);
    expect(scrolled).toHaveBeenCalledWith("message-U3");
    expect(current()).toBe(2);
    // The click leaves no focus behind to hold the list open once the pointer leaves.
    expect(target).not.toHaveFocus();
    window.dispatchEvent(new Event("scroll"));
    await new Promise((resolve) => requestAnimationFrame(resolve));
    expect(current()).toBe(2);
    window.dispatchEvent(new WheelEvent("wheel"));
    await waitFor(() => expect(current()).toBe(5));

    await user.click(within(fork(4)).getByRole("button", { name: "第 2 支" }));
    expect(lines()).toEqual(
      ["U1", "A1", "U3", "A3", "U4", "A4"].map((id) => `目录 ${id}`),
    );
  } finally {
    // @ts-expect-error jsdom has none to restore.
    delete Element.prototype.scrollIntoView;
  }
});

it("holds the header's place with a hidden skeleton until the conversation arrives", async () => {
  let arrive!: (response: Response) => void;
  vi.spyOn(globalThis, "fetch").mockImplementation((url) =>
    requestPath(url).endsWith("/annotations")
      ? Promise.resolve(Response.json({ items: [] }))
      : new Promise((resolve) => (arrive = resolve)),
  );
  mountApp("/conversations/tree?path=p2&date=2025-10-02");
  const status = await screen.findByRole("status");
  expect(screen.getByRole("link", { name: /10 月 2 日/ })).toHaveAttribute(
    "href",
    "/?date=2025-10-02&moment=p2",
  );
  expect(status).toHaveTextContent("正在加载对话…");
  expect(screen.queryByRole("heading")).not.toBeInTheDocument();
  expect(status.closest("header")?.querySelector(".skeleton")).toHaveAttribute(
    "aria-hidden",
    "true",
  );
  arrive(Response.json(tree()));
  expect(
    await screen.findByRole("heading", { name: "第 2 支" }),
  ).toBeInTheDocument();
  expect(screen.queryByRole("status")).not.toBeInTheDocument();
});
