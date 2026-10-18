import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { expect, it, vi } from "vitest";
import { KeyManager } from "./key-manager";
import { requestPath, testRequest } from "@/lib/test-request";
import type { AccessToken } from "@/lib/api";

const token = `palace_pat_Ab3d${"x".repeat(35)}wXyZ`;
const masked = `palace_pat_Ab3d${"*".repeat(35)}wXyZ`;
const used: AccessToken = {
  id: "used",
  note: "同步脚本",
  masked: `palace_pat_Qq11${"*".repeat(35)}Zz99`,
  created_at: new Date(2026, 9, 1, 8).getTime(),
  last_used_at: new Date(2026, 9, 17, 22).getTime(),
};

function setup(initial: AccessToken[] = [used]) {
  let tokens = structuredClone(initial);
  const posted: unknown[] = [];
  const fetch = vi
    .spyOn(globalThis, "fetch")
    .mockImplementation(async (input) => {
      const request = testRequest(input);
      if (request.method === "POST") {
        const body = (await request.json()) as { note: string };
        posted.push(body);
        const { note } = body;
        const created: AccessToken = {
          id: "fresh",
          note,
          masked,
          created_at: new Date(2026, 9, 18, 9).getTime(),
          last_used_at: null,
        };
        tokens = [created, ...tokens];
        return Response.json({ access_token: created, token }, { status: 201 });
      }
      if (request.method === "DELETE") {
        tokens = tokens.filter(
          ({ id }) => !requestPath(input).endsWith(`/${id}`),
        );
        return new Response(null, { status: 204 });
      }
      return Response.json(tokens);
    });
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  const view = () => (
    <QueryClientProvider client={client}>
      <KeyManager onClose={() => {}} />
    </QueryClientProvider>
  );
  const { unmount } = render(view());
  return { client, fetch, posted, unmount, view, user: userEvent.setup() };
}

// Core test case: `specs/test-cases/server/owner/access-tokens.md#the-key-manager-must-never-show-a-full-token`
it("lists keys by note, masked, with when they were made and last used", async () => {
  setup([used, { ...used, id: "idle", note: "闲置", last_used_at: null }]);
  const list = await screen.findByRole("list", { name: "密钥" });
  expect(
    within(list)
      .getAllByRole("listitem")
      .map((item) =>
        [".branch-title", ".key-masked", ".branch-meta"].map(
          (part) => item.querySelector(part)!.textContent,
        ),
      ),
  ).toEqual([
    ["同步脚本", used.masked, "创建于 2026-10-01 · 最近使用 2026-10-17"],
    ["闲置", used.masked, "创建于 2026-10-01 · 从未使用"],
  ]);
  expect(screen.queryByRole("button", { name: "复制密钥" })).toBeNull();
});

// Core test case: `specs/test-cases/server/owner/access-tokens.md#the-key-manager-must-never-show-a-full-token`
it("creates a key it shows only masked, copies it in full once, and forgets it on close", async () => {
  const { client, posted, unmount, view, user } = setup([]);
  expect(await screen.findByText("还没有密钥。")).toBeInTheDocument();
  await user.type(screen.getByLabelText("备注"), " 命令行 ");
  await user.click(screen.getByRole("button", { name: "生成" }));
  const row = (await screen.findByText(masked)).closest("li")!;
  expect(row).toHaveTextContent("仅此一次可复制，关闭后无法再次查看");
  expect(screen.getByLabelText("备注")).toHaveValue("");
  expect(posted).toEqual([{ note: " 命令行 " }]);

  const writeText = vi.spyOn(navigator.clipboard, "writeText");
  await user.click(within(row).getByRole("button", { name: "复制密钥" }));
  expect(writeText).toHaveBeenLastCalledWith(token);
  expect(
    within(row).getByRole("button", { name: "复制密钥" }),
  ).toHaveTextContent("已复制");
  // The full value is on the clipboard only: not on the page, nor in the query cache.
  expect(document.body.textContent).not.toContain(token);
  expect(
    JSON.stringify(
      client
        .getQueryCache()
        .getAll()
        .map((q) => q.state.data),
    ),
  ).not.toContain(token);

  unmount();
  render(view());
  const reopened = (await screen.findByText(masked)).closest("li")!;
  expect(
    within(reopened).queryByRole("button", { name: "复制密钥" }),
  ).toBeNull();
  expect(reopened).toHaveTextContent("创建于 2026-10-18 · 从未使用");
});

// Core test case: `specs/test-cases/server/owner/access-tokens.md#the-key-manager-must-never-show-a-full-token`
it("deletes a key only after confirming", async () => {
  const { fetch, user } = setup();
  await user.click(
    await screen.findByRole("button", { name: "删除密钥 同步脚本" }),
  );
  const confirm = await screen.findByRole("dialog", { name: "删除密钥？" });
  expect(confirm).toHaveTextContent("使用“同步脚本”的程序将立即无法访问");
  expect(
    fetch.mock.calls.some(([input]) => testRequest(input).method === "DELETE"),
  ).toBe(false);
  await user.click(
    within(confirm).getByRole("button", { name: "确认删除密钥" }),
  );
  expect(await screen.findByText("还没有密钥。")).toBeInTheDocument();
  await waitFor(() =>
    expect(screen.queryByRole("dialog", { name: "删除密钥？" })).toBeNull(),
  );
  const deleted = fetch.mock.calls
    .map(([input]) => testRequest(input))
    .find((request) => request.method === "DELETE")!;
  expect(new URL(deleted.url).pathname).toBe("/api/access-tokens/used");
});
