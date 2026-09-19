import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, expect, it, vi } from "vitest";
import { App } from "./app";
import { useLibrary } from "./lib/store";

const old = {
  id: "old",
  title: "较早的思考",
  source: "chatgpt",
  session_id: "session-old",
  occurred_at: 1000,
  head_message_id: "head-old",
  message_count: 2,
};
const latest = {
  ...old,
  id: "new",
  title: "新的灵感",
  occurred_at: 3000,
  head_message_id: "head-new",
};
const messages = [
  {
    id: "a",
    role: "user",
    content: "你好",
    parent_message_id: null,
    created_order: 1,
  },
  {
    id: "head-new",
    role: "assistant",
    content:
      "**回答**\n\n| A | B |\n|---|---|\n|1|2|\n\n<script>alert(1)</script>\n\n[危险](javascript:alert(1))",
    parent_message_id: "a",
    created_order: 2,
  },
];
function mount(route = "/") {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={[route]}>
        <App />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}
beforeEach(() => {
  act(() => useLibrary.setState({ search: "" }));
});
function mockApi() {
  return vi.spyOn(globalThis, "fetch").mockImplementation(async (url) => {
    const path = String(url);
    if (path === "/api/conversations") return Response.json([old, latest]);
    if (path === "/api/import/file")
      return Response.json({
        conversation_id: "new",
        head_message_id: "head-new",
      });
    if (path.includes("/paths/")) return Response.json(messages);
    return Response.json({
      conversation: latest,
      messages,
      original_link: "https://chatgpt.com/c/session-old",
    });
  });
}
it("sorts by conversation occurrence time, searches, navigates, and safely renders Markdown", async () => {
  mockApi();
  mount();
  const user = userEvent.setup();
  await screen.findByText("新的灵感");
  expect(
    screen.getAllByRole("heading", { level: 2 }).map((h) => h.textContent),
  ).toEqual(["新的灵感", "较早的思考"]);
  await user.type(screen.getByRole("textbox", { name: "搜索会话" }), "新的");
  expect(screen.queryByText("较早的思考")).not.toBeInTheDocument();
  await user.click(screen.getByText("新的灵感"));
  expect(await screen.findByText("回答")).toHaveProperty("tagName", "STRONG");
  expect(screen.getByRole("table")).toBeInTheDocument();
  expect(document.querySelector("script")).toBeNull();
  expect(screen.getByText("危险")).not.toHaveAttribute(
    "href",
    expect.stringContaining("javascript:"),
  );
  await user.click(screen.getByRole("link", { name: "返回会话收藏" }));
  expect(await screen.findByRole("textbox", { name: "搜索会话" })).toHaveValue(
    "新的",
  );
});
it("shows authentication and network failures with a retry action", async () => {
  const fetch = vi
    .spyOn(globalThis, "fetch")
    .mockResolvedValueOnce(Response.json({}, { status: 503 }))
    .mockResolvedValueOnce(Response.json({}, { status: 401 }));
  mount();
  const user = userEvent.setup();
  expect(await screen.findByRole("alert")).toHaveTextContent("服务暂时不可用");
  await user.click(screen.getByRole("button", { name: "重试" }));
  expect(
    await screen.findByRole("link", { name: "前往登录 →" }),
  ).toHaveAttribute("href", "/auth/login");
  expect(fetch).toHaveBeenCalledTimes(2);
});
it("imports the selected source, ID, title, local date/time and JSON as multipart", async () => {
  const fetch = mockApi();
  mount();
  const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: "导入会话" }));
  const dialog = await screen.findByRole("dialog");
  await user.selectOptions(within(dialog).getByLabelText("会话来源"), "gemini");
  await user.type(
    within(dialog).getByLabelText("来源网站 Session ID"),
    "my-session",
  );
  await user.type(within(dialog).getByLabelText("自定义标题"), "我的收藏");
  const now = new Date();
  expect(
    within(dialog).getByRole("button", { name: "选择对话发生日期" }),
  ).toHaveTextContent(`${now.getFullYear()} 年`);
  // A real calendar selection keeps the time in the browser's local timezone.
  await user.click(
    within(dialog).getByRole("button", { name: "选择对话发生日期" }),
  );
  expect(screen.getByRole("grid")).toBeInTheDocument();
  await user.keyboard("{Escape}");
  const file = new File(['[{"role":"user","content":"hello"}]'], "chat.json", {
    type: "application/json",
  });
  await user.upload(within(dialog).getByLabelText("选择会话 JSON 文件"), file);
  await user.click(
    within(dialog).getByRole("button", { name: "导入并查看会话" }),
  );
  await screen.findByText("回答");
  const call = fetch.mock.calls.find(([url]) => url === "/api/import/file")!;
  const body = call[1]!.body as FormData;
  expect(
    Object.fromEntries(
      [...body.entries()].filter(
        ([key]) => !["history", "idempotency_key", "occurred_at"].includes(key),
      ),
    ),
  ).toEqual({ source: "gemini", title: "我的收藏", session_id: "my-session" });
  expect(body.get("history")).toBe(file);
  expect(
    Math.abs(Number(body.get("occurred_at")) - now.getTime()),
  ).toBeLessThan(10_000);
  expect(body.get("idempotency_key")).toEqual(expect.any(String));
});
it("rejects malformed JSON before sending and preserves the form for correction", async () => {
  const fetch = mockApi();
  mount();
  const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: "导入会话" }));
  await user.type(await screen.findByLabelText("来源网站 Session ID"), "s");
  await user.type(screen.getByLabelText("自定义标题"), "保留标题");
  await user.upload(
    screen.getByLabelText("选择会话 JSON 文件"),
    new File(["{"], "bad.json", { type: "application/json" }),
  );
  await user.click(screen.getByRole("button", { name: "导入并查看会话" }));
  expect(await screen.findByRole("alert")).toHaveTextContent(
    "文件不是有效的 JSON",
  );
  expect(screen.getByLabelText("自定义标题")).toHaveValue("保留标题");
  expect(fetch.mock.calls.some(([url]) => url === "/api/import/file")).toBe(
    false,
  );
});
it("reuses an idempotency key on retry after an ambiguous failure", async () => {
  const keys: FormDataEntryValue[] = [];
  let fail = true;
  const fetch = mockApi();
  const fallback = fetch.getMockImplementation()!;
  fetch.mockImplementation(async (url, options) => {
    if (url === "/api/import/file") {
      keys.push((options!.body as FormData).get("idempotency_key")!);
      if (fail) {
        fail = false;
        throw new Error("网络中断");
      }
    }
    return fallback(url, options);
  });
  mount();
  const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: "导入会话" }));
  await user.type(await screen.findByLabelText("来源网站 Session ID"), "s");
  await user.type(screen.getByLabelText("自定义标题"), "retry");
  await user.upload(
    screen.getByLabelText("选择会话 JSON 文件"),
    new File(['[{"role":"user","content":"x"}]'], "chat.json"),
  );
  await user.click(screen.getByRole("button", { name: "导入并查看会话" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("网络中断");
  await user.click(screen.getByRole("button", { name: "导入并查看会话" }));
  await screen.findByText("回答");
  await waitFor(() => expect(keys).toHaveLength(2));
  expect(keys[0]).toBe(keys[1]);
});
