import { screen, waitFor, within } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { address, mountApp } from "@/test-app";
import type { Detail, Message } from "@/lib/api";
import type { components } from "@/lib/generated/api";
import { requestPath, testRequest } from "@/lib/test-request";
import type { ComponentType, ReactNode } from "react";

// Browser tests cover the measured canvas; here the branch cards render without layout.
vi.mock("@xyflow/react", () => ({
  ReactFlow: ({
    nodes,
    nodeTypes,
    children,
  }: {
    nodes: { id: string; data: unknown }[];
    nodeTypes: { branch: ComponentType<{ data: unknown }> };
    children: ReactNode;
  }) => (
    <div>
      {nodes.map((node) => (
        <nodeTypes.branch key={node.id} data={node.data} />
      ))}
      {children}
    </div>
  ),
  Background: () => null,
  Controls: () => null,
  Handle: () => null,
  Position: { Top: "top", Bottom: "bottom" },
  BackgroundVariant: { Dots: "dots" },
}));

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
      if (path === "/api/timeline") return Response.json([]);
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

// Core test case: `specs/test-cases/server/conversation/message-tree.md#fork-selection-must-resolve-to-one-real-source-session`
it("defaults to the latest path and resets downstream forks to the latest matching continuation", async () => {
  const { user } = setup("/conversations/tree");
  await screen.findByText("A5");
  expect(screen.getByRole("link", { name: "继续对话 ↗" })).toHaveAttribute(
    "href",
    "https://chatgpt.com/c/s3",
  );
  await user.click(within(fork(4)).getByRole("button", { name: "U4" }));
  expect(screen.getByText("A4")).toBeInTheDocument();
  expect(screen.queryByText("A5")).not.toBeInTheDocument();
  expect(screen.getByRole("link", { name: "继续对话 ↗" })).toHaveAttribute(
    "href",
    "https://chatgpt.com/c/s2",
  );
  await user.click(within(fork(2)).getByRole("button", { name: "U2" }));
  expect(screen.getByText("A2")).toBeInTheDocument();
  expect(
    screen.queryByRole("group", { name: "第 4 条消息后的分支" }),
  ).not.toBeInTheDocument();
  await user.click(within(fork(2)).getByRole("button", { name: "U3" }));
  expect(screen.getByText("A5")).toBeInTheDocument();
  expect(within(fork(4)).getByRole("button", { name: "U5" })).toHaveAttribute(
    "aria-current",
    "true",
  );
});

// Core test case: `specs/test-cases/server/conversation/message-tree.md#fork-selection-must-resolve-to-one-real-source-session`
it("deep-links to internal endpoints and distinguishes sessions with identical message paths", async () => {
  const { user } = setup("/conversations/tree?path=p4");
  await screen.findByText("A1");
  expect(document.querySelectorAll(".message")).toHaveLength(2);
  expect(fork(2)).toHaveTextContent("此处分为 4 支：");
  expect(
    within(fork(2)).getByRole("button", { name: "在此结束 · s4" }),
  ).toHaveAttribute("aria-current", "true");
  await user.click(
    within(fork(2)).getByRole("button", { name: "在此结束 · s5" }),
  );
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
  await user.click(within(fork(2)).getByRole("button", { name: "U2" }));
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
  ).toEqual(["编辑对话", "分支管理", "删除对话"]);
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
  await user.click(within(fork(2)).getByRole("button", { name: "U3" }));
  expect(
    await screen.findByRole("heading", { level: 1, name: "第 3 支" }),
  ).toBeInTheDocument();
});

// Core test case: `specs/test-cases/server/conversation/reading-page.md#deleting-a-conversation-must-return-to-where-the-reader-came-from`
it.each([
  ["/conversations/tree?path=p1&date=2025-09-30", "/?date=2025-09-30"],
  ["/conversations/tree?path=p1", "/"],
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
  await choose(user, "分支管理");
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
  await choose(user, "分支管理");
  await user.click(await screen.findByRole("button", { name: "新建分支" }));
  const form = await screen.findByRole("dialog", { name: "新建分支" });
  // A branch starts with the title of the path being read, and may be renamed.
  expect(within(form).getByLabelText("标题")).toHaveValue("第 1 支");
  await user.type(within(form).getByLabelText("Session ID"), "s9");
  await user.upload(
    within(form).getByLabelText("选择对话 JSON 文件"),
    new File(['[{"role":"user","content":"U1"}]'], "branch.json"),
  );
  await user.click(within(form).getByRole("button", { name: "导入分支" }));
  await waitFor(() => expect(router.state.location.search).toContain("p5"));
  const today = new Date();
  const day = `${today.getFullYear()}-${String(today.getMonth() + 1).padStart(2, "0")}-${String(today.getDate()).padStart(2, "0")}`;
  expect(address(router)).toBe(`/conversations/tree?path=p5&date=${day}`);
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
});
