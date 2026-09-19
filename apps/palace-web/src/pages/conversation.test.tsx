import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import { expect, it, vi } from "vitest";
import { App } from "@/app";
import type { Detail, Message } from "@/lib/api";

const chains = [
  ["U1", "A1", "U2", "A2"],
  ["U1", "A1", "U3", "A3", "U4", "A4"],
  ["U1", "A1", "U3", "A3", "U5", "A5"],
  ["U1", "A1"],
  ["U1", "A1"],
];
function setup(route = "/conversations/tree") {
  const messages = new Map<string, Message>();
  for (const chain of chains)
    chain.forEach((id, index) =>
      messages.set(id, {
        id,
        content: id,
        role: id.startsWith("U") ? "user" : "assistant",
        parent_message_id: chain[index - 1] ?? null,
        created_order: index,
      }),
    );
  const detail: Detail = {
    conversation: { id: "tree", title: "嵌套分支", source: "chatgpt" },
    messages: [...messages.values()],
    paths: chains.map((chain, i) => ({
      id: `p${i + 1}`,
      session_id: `s${i + 1}`,
      head_message_id: chain.at(-1)!,
      message_count: chain.length,
      occurred_at: 1000,
      created_at: 1000,
      updated_at: [1000, 3000, 4000, 2000, 2000][i],
      original_link: `https://chatgpt.com/c/s${i + 1}`,
    })),
  };
  vi.spyOn(globalThis, "fetch").mockResolvedValue(Response.json(detail));
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={[route]}>
        <App />
      </MemoryRouter>
    </QueryClientProvider>,
  );
  return userEvent.setup();
}
// Core test case: `specs/test-cases/server/conversation/message-tree.md#fork-selection-must-resolve-to-one-real-source-session`
it("defaults to the latest path and resets downstream forks to the latest matching continuation", async () => {
  const user = setup();
  await screen.findByText("A5");
  expect(screen.getByRole("link", { name: "继续对话" })).toHaveAttribute(
    "href",
    "https://chatgpt.com/c/s3",
  );
  await user.selectOptions(
    screen.getByRole("combobox", { name: "第 4 条消息后的分支" }),
    "message:U4",
  );
  expect(screen.getByText("A4")).toBeInTheDocument();
  expect(screen.queryByText("A5")).not.toBeInTheDocument();
  expect(screen.getByRole("link", { name: "继续对话" })).toHaveAttribute(
    "href",
    "https://chatgpt.com/c/s2",
  );
  await user.selectOptions(
    screen.getByRole("combobox", { name: "第 2 条消息后的分支" }),
    "message:U2",
  );
  expect(screen.getByText("A2")).toBeInTheDocument();
  expect(
    screen.queryByRole("combobox", { name: "第 4 条消息后的分支" }),
  ).not.toBeInTheDocument();
  await user.selectOptions(
    screen.getByRole("combobox", { name: "第 2 条消息后的分支" }),
    "message:U3",
  );
  expect(screen.getByText("A5")).toBeInTheDocument();
  expect(
    screen.getByRole("combobox", { name: "第 4 条消息后的分支" }),
  ).toHaveValue("message:U5");
  expect(screen.getByRole("link", { name: "继续对话" })).toHaveAttribute(
    "href",
    "https://chatgpt.com/c/s3",
  );
});
// Core test case: `specs/test-cases/server/conversation/message-tree.md#fork-selection-must-resolve-to-one-real-source-session`
it("deep-links to internal endpoints and distinguishes sessions with identical message paths", async () => {
  const user = setup("/conversations/tree?path=p4");
  await screen.findByText("A1");
  expect(document.querySelectorAll(".message")).toHaveLength(2);
  const fork = screen.getByRole("combobox", { name: "第 2 条消息后的分支" });
  expect(
    within(fork).getByRole("option", { name: "在此结束 · s4" }),
  ).toBeInTheDocument();
  expect(
    within(fork).getByRole("option", { name: "在此结束 · s5" }),
  ).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "继续对话" })).toHaveAttribute(
    "href",
    "https://chatgpt.com/c/s4",
  );
  await user.selectOptions(fork, "path:p5");
  expect(document.querySelectorAll(".message")).toHaveLength(2);
  expect(screen.getByRole("link", { name: "继续对话" })).toHaveAttribute(
    "href",
    "https://chatgpt.com/c/s5",
  );
});
