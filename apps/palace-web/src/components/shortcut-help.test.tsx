import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it } from "vitest";
import {
  ShortcutHelp,
  useShortcutHelp,
  type ShortcutGroup,
} from "./shortcut-help";

const page: ShortcutGroup = {
  title: "时刻",
  shortcuts: [
    { keys: ["←"], action: "前一天" },
    { keys: ["→"], action: "后一天" },
  ],
};

function Page() {
  useShortcutHelp(page);
  return <input aria-label="搜索" />;
}

// The panel as a reader scans it: each group title with its rows of keys and action.
function panelContents() {
  const panel = screen.getByRole("dialog", { name: "快捷键" });
  return within(panel)
    .getAllByRole("heading", { level: 3 })
    .map((heading) => ({
      title: heading.textContent,
      rows: [...heading.nextElementSibling!.children].map((row) => [
        [...row.querySelectorAll("kbd")].map((key) => key.textContent),
        row.querySelector("dd")!.textContent,
      ]),
    }));
}

it("toggles with ? and lists the mounted page's keys before the general ones", async () => {
  const user = userEvent.setup();
  render(
    <ShortcutHelp>
      <Page />
    </ShortcutHelp>,
  );
  await user.keyboard("?");
  expect(await screen.findByRole("dialog", { name: "快捷键" })).toBeVisible();
  expect(panelContents()).toEqual([
    {
      title: "时刻",
      rows: [
        [["←"], "前一天"],
        [["→"], "后一天"],
      ],
    },
    { title: "通用", rows: [[["?"], "显示或关闭快捷键"]] },
  ]);
  await user.keyboard("?");
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
});

it("drops a page's keys once it unmounts", async () => {
  const user = userEvent.setup();
  const { rerender } = render(
    <ShortcutHelp>
      <Page />
    </ShortcutHelp>,
  );
  rerender(<ShortcutHelp>{null}</ShortcutHelp>);
  await user.keyboard("?");
  await screen.findByRole("dialog", { name: "快捷键" });
  expect(panelContents()).toEqual([
    { title: "通用", rows: [[["?"], "显示或关闭快捷键"]] },
  ]);
});

it("leaves ? to a text field being typed in", async () => {
  const user = userEvent.setup();
  render(
    <ShortcutHelp>
      <Page />
    </ShortcutHelp>,
  );
  await user.type(screen.getByRole("textbox", { name: "搜索" }), "?");
  expect(screen.getByRole("textbox", { name: "搜索" })).toHaveValue("?");
  expect(screen.queryByRole("dialog")).toBeNull();
});
