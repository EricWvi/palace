import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import { expect, it, vi } from "vitest";
import { BranchManager } from "./branch-manager";
import { ConversationActions } from "./conversation-actions";
import { testRequest, requestPath } from "@/lib/test-request";
import type { components } from "@/lib/generated/api";
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
    title: "一棵树",
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
function setup(menu = false) {
  let current = structuredClone(detail);
  const fetch = vi
    .spyOn(globalThis, "fetch")
    .mockImplementation(async (url) => {
      const request = testRequest(url);
      if (
        testRequest(url).method === "PUT" &&
        requestPath(url) === "/api/conversations/tree"
      ) {
        const metadata: components["schemas"]["ConversationMetadata"] =
          await request.clone().json();
        current = {
          ...current,
          conversation: { ...current.conversation, ...metadata },
        };
        return Response.json(current.conversation);
      }
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
        {menu ? (
          <ConversationActions conversation={detail.conversation} />
        ) : (
          <BranchManager
            conversation={detail.conversation}
            onClose={() => {}}
          />
        )}
      </MemoryRouter>
    </QueryClientProvider>,
  );
  return { fetch, user: userEvent.setup() };
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
it("creates branches with disabled metadata and sends only the conversation target and path input", async () => {
  const { fetch, user } = setup();
  await user.click(await screen.findByRole("button", { name: "新建分支" }));
  const form = await screen.findByRole("dialog", { name: "新建分支" });
  expect(within(form).getByLabelText("自定义标题")).toBeDisabled();
  expect(within(form).getByLabelText("会话来源")).toBeDisabled();
  await user.type(within(form).getByLabelText("来源网站 Session ID"), "s4");
  const history = '[{"role":"user","content":"hello"}]';
  await user.upload(
    within(form).getByLabelText("选择会话 JSON 文件"),
    new File([history], "branch.json"),
  );
  await user.click(within(form).getByRole("button", { name: "导入分支" }));
  await waitFor(() =>
    expect(
      screen.queryByRole("dialog", { name: "新建分支" }),
    ).not.toBeInTheDocument(),
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
      session_id: "s4",
      history,
      occurred_at: expect.any(Number),
      idempotency_key: expect.any(String),
    },
  ]);
});
it("updates using the original occurrence time without sending disabled identity fields", async () => {
  const { fetch, user } = setup();
  await user.click(await screen.findByRole("button", { name: "更新分支 s1" }));
  const form = await screen.findByRole("dialog", { name: "更新分支" });
  expect(
    within(form).getByRole("button", { name: "选择对话发生日期" }),
  ).toHaveTextContent("2024 年 03 月 05 日");
  expect(within(form).getByLabelText("对话发生时间")).toHaveValue("09:30");
  expect(within(form).getByLabelText("来源网站 Session ID")).toBeDisabled();
  const history = '[{"role":"user","content":"hello"}]';
  await user.upload(
    within(form).getByLabelText("选择会话 JSON 文件"),
    new File([history], "update.json"),
  );
  await user.click(within(form).getByRole("button", { name: "保存更新" }));
  await waitFor(() =>
    expect(
      screen.queryByRole("dialog", { name: "更新分支" }),
    ).not.toBeInTheDocument(),
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
it("offers branch management and confirmed whole-conversation deletion from the card menu", async () => {
  const { fetch, user } = setup(true);
  await user.tab();
  await user.keyboard("{Enter}");
  expect(
    screen.getByRole("menuitem", { name: "分支管理" }),
  ).toBeInTheDocument();
  await user.click(screen.getByRole("menuitem", { name: "删除对话" }));
  await user.click(await screen.findByRole("button", { name: "确认删除对话" }));
  await waitFor(() =>
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
  );
  expect(fetch).toHaveBeenCalledWith(
    expect.objectContaining({
      url: new URL("/api/conversations/tree", window.location.origin).href,
      method: "DELETE",
    }),
  );
});
// Core test case: `specs/test-cases/server/conversation/message-tree.md#card-menu-metadata-editing-must-refresh-every-visible-projection`
it("edits complete conversation metadata from the card menu", async () => {
  const { fetch, user } = setup(true);
  await user.tab();
  await user.keyboard("{Enter}");
  await user.click(screen.getByRole("menuitem", { name: "编辑会话" }));
  const dialog = await screen.findByRole("dialog", { name: "编辑会话" });
  const title = within(dialog).getByLabelText("会话标题");
  const source = within(dialog).getByLabelText("消息来源");
  expect(title).toHaveValue("一棵树");
  expect(source).toHaveValue("chatgpt");
  await user.click(within(dialog).getByRole("button", { name: "取消" }));
  expect(screen.queryByRole("dialog", { name: "编辑会话" })).toBeNull();
  expect(
    fetch.mock.calls.some(
      ([url]) =>
        requestPath(url) === "/api/conversations/tree" &&
        testRequest(url).method === "PUT",
    ),
  ).toBe(false);
  screen
    .getByRole("button", { name: `会话菜单 ${detail.conversation.title}` })
    .focus();
  await user.keyboard("{Enter}");
  await user.click(screen.getByRole("menuitem", { name: "编辑会话" }));
  const reopened = await screen.findByRole("dialog", { name: "编辑会话" });
  const reopenedTitle = within(reopened).getByLabelText("会话标题");
  const reopenedSource = within(reopened).getByLabelText("消息来源");
  await user.clear(reopenedTitle);
  await user.type(reopenedTitle, "   ");
  await user.click(within(reopened).getByRole("button", { name: "保存更改" }));
  expect(await within(reopened).findByRole("alert")).toHaveTextContent(
    "请填写会话标题",
  );
  await user.clear(reopenedTitle);
  await user.type(reopenedTitle, "修正后的树");
  await user.selectOptions(reopenedSource, "gemini");
  const fallback = fetch.getMockImplementation()!;
  let release: (() => void) | undefined;
  fetch.mockImplementation(async (url) => {
    if (
      requestPath(url) === "/api/conversations/tree" &&
      testRequest(url).method === "PUT"
    ) {
      return new Promise<Response>((resolve) => {
        release = () =>
          resolve(
            Response.json({
              id: "tree",
              title: "修正后的树",
              source: "gemini",
            }),
          );
      });
    }
    return fallback(url);
  });
  await user.click(within(reopened).getByRole("button", { name: "保存更改" }));
  await waitFor(() =>
    expect(
      within(reopened).getByRole("button", { name: "正在保存…" }),
    ).toBeDisabled(),
  );
  await act(async () => release!());
  await waitFor(() =>
    expect(
      screen.queryByRole("dialog", { name: "编辑会话" }),
    ).not.toBeInTheDocument(),
  );
  const saved = testRequest(
    fetch.mock.calls.find(([url]) => testRequest(url).method === "PUT")![0],
  );
  expect([
    requestPath(saved),
    saved.headers.get("Content-Type"),
    await saved.clone().json(),
  ]).toEqual([
    "/api/conversations/tree",
    "application/json",
    { title: "修正后的树", source: "gemini" },
  ]);
});
