import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useState } from "react";
import { expect, it, vi } from "vitest";
import { requestPath, testRequest } from "@/lib/test-request";
import { OverallNote } from "./overall-note";

const target = { kind: "article", id: "a1" } as const;
const url = "/api/articles/a1/overall-note";

// Serves one note; `refuse` is the status of the next saves, when they should fail.
function serve(initial: string | null, version = initial ? 5 : 0) {
  const state = { text: initial, version, refuse: null as number | null };
  const saves: unknown[] = [];
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
    const request = testRequest(input);
    expect(requestPath(input)).toBe(url);
    if (request.method === "GET")
      return Response.json({ text: state.text, version: state.version });
    const body = (await request.json()) as { text: string; version: number };
    saves.push(body);
    if (state.refuse)
      return Response.json(
        { error: "overall_note_changed", login: "" },
        { status: state.refuse },
      );
    state.text = body.text.trim() ? body.text : null;
    state.version = state.text ? state.version + 1 : 0;
    return Response.json({ text: state.text, version: state.version });
  });
  return { state, saves };
}

function Page() {
  const [input] = useState(
    () => new QueryClient({ defaultOptions: { queries: { retry: false } } }),
  );
  return (
    <QueryClientProvider client={input}>
      <input aria-label="别处" />
      <OverallNote target={target} images={{ kind: "placeholder" }} />
    </QueryClientProvider>
  );
}

// Lets the query settle, so `n` has a version to write from.
async function mount() {
  const user = userEvent.setup();
  render(<Page />);
  await waitFor(() => expect(globalThis.fetch).toHaveBeenCalled());
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve));
  });
  return user;
}

// Core test case: `specs/test-cases/web/stars/overall-notes.md#the-overall-note-card-must-stay-out-of-the-reading-flow`
it("shows nothing without a note, and writes the first one with n", async () => {
  const { saves } = serve(null);
  const user = await mount();
  expect(screen.queryByRole("complementary")).toBeNull();
  await user.keyboard("n");
  const editor = await screen.findByRole("textbox", { name: "总批" });
  await user.keyboard("想到一件事");
  await user.keyboard("{Escape}");
  expect(editor).not.toHaveFocus();
  expect(
    await screen.findByRole("button", { name: /想到一件事/ }),
  ).toBeInTheDocument();
  expect(saves).toEqual([{ text: "想到一件事", version: 0 }]);
});

// Core test case: `specs/test-cases/web/stars/overall-notes.md#the-overall-note-card-must-stay-out-of-the-reading-flow`
it("saves by itself once typing pauses, naming the version it last saw", async () => {
  const { saves } = serve("原来的");
  const user = await mount();
  await user.keyboard("n");
  await screen.findByRole("textbox", { name: "总批" });
  await user.keyboard("，又一句");
  await waitFor(() => expect(saves).toHaveLength(1), { timeout: 2000 });
  expect(saves).toEqual([{ text: "原来的，又一句", version: 5 }]);
  await user.keyboard("！{Escape}");
  await screen.findByRole("button", { name: /原来的，又一句！/ });
  expect(saves[1]).toEqual({ text: "原来的，又一句！", version: 6 });
});

// Core test case: `specs/test-cases/web/stars/overall-notes.md#the-overall-note-card-must-stay-out-of-the-reading-flow`
it("keeps the card and the words open when a save fails", async () => {
  const { state } = serve("原来的");
  state.refuse = 409;
  const user = await mount();
  await user.keyboard("n");
  const editor = await screen.findByRole("textbox", { name: "总批" });
  await user.keyboard("，写到一半{Escape}");
  expect(await screen.findByRole("alert")).toHaveTextContent(
    "总批在别处改过了",
  );
  expect(editor).toHaveValue("原来的，写到一半");
  expect(state.text).toBe("原来的");
});

// Core test case: `specs/test-cases/web/stars/overall-notes.md#the-overall-note-card-must-stay-out-of-the-reading-flow`
it("leaves n alone in a text field and while a dialog owns the keyboard", async () => {
  serve("原来的");
  const user = await mount();
  await user.click(screen.getByRole("textbox", { name: "别处" }));
  await user.keyboard("n");
  expect(screen.getByRole("textbox", { name: "别处" })).toHaveValue("n");
  expect(screen.queryByRole("textbox", { name: "总批" })).toBeNull();
});

// Core test case: `specs/test-cases/web/stars/overall-notes.md#the-overall-note-card-must-stay-out-of-the-reading-flow`
it("reads the note rendered from its fold, and folds again on Escape", async () => {
  serve("第一行\n\n**第二段**");
  const user = await mount();
  await user.click(await screen.findByRole("button", { name: /第一行/ }));
  expect((await screen.findByText("第二段")).tagName).toBe("STRONG");
  await user.keyboard("{Escape}");
  expect(
    await screen.findByRole("button", { name: /第一行/ }),
  ).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: /第一行/ }));
  await user.click(await screen.findByText("第二段"));
  expect(await screen.findByRole("textbox", { name: "总批" })).toHaveValue(
    "第一行\n\n**第二段**",
  );
  await user.keyboard("{Escape}");
  await screen.findByRole("button", { name: /第一行/ });
});
