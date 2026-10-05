import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import { expect, it, vi } from "vitest";
import { BranchManager } from "./branch-manager";
import { testRequest, requestPath } from "@/lib/test-request";
import type { Detail } from "@/lib/api";
import type { ComponentType, ReactNode } from "react";

// Browser tests cover the measured canvas; these tests exercise real cards and form actions.
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

const detail: Detail = {
  conversation: {
    owner_id: "owner",
    id: "tree",
    source: "chatgpt",
  },
  messages: [
    {
      id: "u1",
      owner_id: "owner",
      conversation_id: "tree",
      parent_message_id: null,
      role: "user",
      content: "Shared 共同问题很长的标题 mixed English 中文".repeat(4),
      created_order: 1,
    },
    {
      id: "a1",
      owner_id: "owner",
      conversation_id: "tree",
      parent_message_id: "u1",
      role: "assistant",
      content: "隐藏的回答",
      created_order: 2,
    },
    {
      id: "u2",
      owner_id: "owner",
      conversation_id: "tree",
      parent_message_id: "a1",
      role: "user",
      content: "后续问题",
      created_order: 3,
    },
  ],
  paths: [
    {
      id: "long",
      title: "长的一支",
      session_id: "s1",
      head_message_id: "u2",
      occurred_at: new Date(2024, 2, 5, 9, 30).getTime(),
      created_at: 1000,
      updated_at: 3000,
      message_count: 3,
      original_link: "https://chatgpt.com/c/s1",
    },
    {
      id: "short",
      title: "短的一支",
      session_id: "s2",
      head_message_id: "a1",
      occurred_at: 1000,
      created_at: 1000,
      updated_at: 2000,
      message_count: 2,
      original_link: "https://chatgpt.com/c/s2",
    },
    {
      id: "same",
      title: "同样的一支",
      session_id: "s3",
      head_message_id: "a1",
      occurred_at: 1000,
      created_at: 1000,
      updated_at: 1000,
      message_count: 2,
      original_link: "https://chatgpt.com/c/s3",
    },
  ],
};
function setup() {
  let current = structuredClone(detail);
  const imported = vi.fn();
  const fetch = vi
    .spyOn(globalThis, "fetch")
    .mockImplementation(async (url) => {
      if (testRequest(url).method === "DELETE") {
        current = {
          ...current,
          paths: current.paths.filter(
            (path) => !requestPath(url).endsWith(`/${path.id}`),
          ),
        };
        return Response.json({ id: "deleted" });
      }
      if (
        testRequest(url).method === "POST" ||
        testRequest(url).method === "PUT"
      )
        return Response.json({
          import_id: "import",
          created: 1,
          reused: 0,
          conversation_id: "tree",
          path_id: "long",
          head_message_id: "u2",
        });
      return Response.json(current);
    });
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  render(
    <QueryClientProvider client={client}>
      <MemoryRouter>
        <BranchManager
          conversationId="tree"
          source="chatgpt"
          title="正在读的一支"
          onImported={imported}
          onClose={() => {}}
        />
      </MemoryRouter>
    </QueryClientProvider>,
  );
  return { fetch, imported, user: userEvent.setup() };
}
// Core test case: `specs/test-cases/server/conversation/message-tree.md#shared-and-internal-endpoint-paths-must-remain-independently-manageable`
it("projects user nodes and exposes separate actions for internal and identical path endpoints", async () => {
  setup();
  await screen.findByText("后续问题");
  expect(screen.getByText(detail.messages[0].content)).toHaveClass(
    "branch-node-text",
  );
  expect(screen.queryByText("隐藏的回答")).not.toBeInTheDocument();
  for (const session of ["s1", "s2", "s3"])
    expect(
      screen.getByRole("button", { name: `更新分支 ${session}` }),
    ).toBeEnabled();
});
// Core test case: `specs/test-cases/server/import/linear-path-import.md#every-import-must-carry-a-valid-path-title`
it("creates a branch named after the path being read, with the source locked to the conversation", async () => {
  const { fetch, imported, user } = setup();
  await user.click(await screen.findByRole("button", { name: "新建分支" }));
  const form = await screen.findByRole("dialog", { name: "新建分支" });
  const title = within(form).getByLabelText("标题");
  expect(title).toHaveValue("正在读的一支");
  expect(within(form).getByLabelText("来源")).toBeDisabled();
  await user.clear(title);
  await user.type(title, "另一种走法");
  await user.type(within(form).getByLabelText("Session ID"), "s4");
  const history = '[{"role":"user","content":"hello"}]';
  await user.upload(
    within(form).getByLabelText("选择对话 JSON 文件"),
    new File([history], "branch.json"),
  );
  await user.click(within(form).getByRole("button", { name: "导入分支" }));
  await waitFor(() =>
    expect(
      screen.queryByRole("dialog", { name: "新建分支" }),
    ).not.toBeInTheDocument(),
  );
  // The page decides where to go next: to the imported path, dated to its occurrence.
  expect(imported).toHaveBeenCalledWith(
    expect.objectContaining({ path_id: "long" }),
    expect.any(Number),
  );
  const call = fetch.mock.calls.find(
    ([url]) => testRequest(url).method === "POST",
  )!;
  expect([
    requestPath(call[0]),
    await testRequest(call[0]).clone().json(),
  ]).toEqual([
    "/api/conversations/tree/paths",
    {
      title: "另一种走法",
      session_id: "s4",
      history,
      occurred_at: expect.any(Number),
      idempotency_key: expect.any(String),
    },
  ]);
});
// Core test case: `specs/test-cases/server/import/linear-path-import.md#every-import-must-carry-a-valid-path-title`
it("updates with the path's own title and occurrence time without sending identity fields", async () => {
  const { fetch, imported, user } = setup();
  await user.click(await screen.findByRole("button", { name: "更新分支 s1" }));
  const form = await screen.findByRole("dialog", { name: "更新分支" });
  expect(
    within(form).getByRole("button", { name: "选择对话发生日期" }),
  ).toHaveTextContent("2024 年 03 月 05 日");
  expect(within(form).getByLabelText("对话发生时间")).toHaveValue("09:30");
  expect(within(form).getByLabelText("Session ID")).toBeDisabled();
  expect(within(form).getByLabelText("标题")).toHaveValue("长的一支");
  const history = '[{"role":"user","content":"hello"}]';
  await user.upload(
    within(form).getByLabelText("选择对话 JSON 文件"),
    new File([history], "update.json"),
  );
  await user.click(within(form).getByRole("button", { name: "保存更新" }));
  await waitFor(() =>
    expect(
      screen.queryByRole("dialog", { name: "更新分支" }),
    ).not.toBeInTheDocument(),
  );
  // The page decides where to go next: to the imported path, dated to its occurrence.
  expect(imported).toHaveBeenCalledWith(
    expect.objectContaining({ path_id: "long" }),
    expect.any(Number),
  );
  const call = fetch.mock.calls.find(
    ([url]) => testRequest(url).method === "PUT",
  )!;
  expect([
    requestPath(call[0]),
    await testRequest(call[0]).clone().json(),
  ]).toEqual([
    "/api/conversations/tree/paths/long",
    {
      title: "长的一支",
      history,
      occurred_at: detail.paths[0].occurred_at,
      idempotency_key: expect.any(String),
    },
  ]);
});
// Core test case: `specs/test-cases/server/conversation/message-tree.md#shared-and-internal-endpoint-paths-must-remain-independently-manageable`
it("confirms path deletion then refreshes the tree", async () => {
  const { fetch, user } = setup();
  await user.click(await screen.findByRole("button", { name: "删除分支 s2" }));
  expect(
    fetch.mock.calls.some(([url]) => testRequest(url).method === "DELETE"),
  ).toBe(false);
  await user.click(screen.getByRole("button", { name: "确认删除分支" }));
  await waitFor(() =>
    expect(
      screen.queryByRole("button", { name: "更新分支 s2" }),
    ).not.toBeInTheDocument(),
  );
  expect(screen.getByRole("button", { name: "更新分支 s3" })).toBeEnabled();
});
