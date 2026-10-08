import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it } from "vitest";
import { MoreMenu } from "./more-menu";

function renderMenu() {
  render(
    <MoreMenu
      label="当天操作"
      actions={[{ label: "导入对话", onSelect() {} }]}
    />,
  );
  return screen.getByRole("button", { name: "当天操作" });
}

// Switching browser tabs blurs the window, which is what makes Radix close the menu.
async function leaveTab() {
  await act(async () => window.dispatchEvent(new Event("blur")));
}

it("parks focus out of sight when a pointer-opened menu closes with the tab", async () => {
  const trigger = renderMenu();
  fireEvent.pointerDown(trigger, { button: 0, ctrlKey: false });
  expect(await screen.findByRole("menu")).toBeInTheDocument();
  await leaveTab();
  expect(screen.queryByRole("menu")).toBeNull();
  // Radix hands focus back on the next tick after the menu unmounts.
  await waitFor(() =>
    expect(document.activeElement).toBe(document.querySelector(".focus-rest")),
  );
});

it("returns focus to the trigger when the keyboard drove the menu", async () => {
  const user = userEvent.setup();
  const trigger = renderMenu();
  trigger.focus();
  await user.keyboard("{Enter}");
  expect(await screen.findByRole("menu")).toBeInTheDocument();
  await leaveTab();
  expect(screen.queryByRole("menu")).toBeNull();
  await waitFor(() => expect(trigger).toHaveFocus());
});

it("reports a pointer or focus reaching the trigger before the menu opens", () => {
  const approaches: string[] = [];
  render(
    <MoreMenu
      label="当天操作"
      actions={[{ label: "导入对话", onSelect() {} }]}
      onApproach={() => approaches.push("approach")}
    />,
  );
  const trigger = screen.getByRole("button", { name: "当天操作" });
  fireEvent.pointerEnter(trigger);
  fireEvent.focus(trigger);
  expect(approaches).toEqual(["approach", "approach"]);
  expect(screen.queryByRole("menu")).toBeNull();
});
