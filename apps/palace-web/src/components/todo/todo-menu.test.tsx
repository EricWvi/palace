import { useState } from "react";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it } from "vitest";
import { MoreEntries } from "./todo-menu";

// An entry that opens a field beside the dots, the way 改名 turns a row's title into one while
// the row's `···` stays where it was.
function RenameRow() {
  const [renaming, setRenaming] = useState(false);
  return (
    <>
      {renaming && (
        <input
          aria-label="任务标题"
          autoFocus
          onBlur={() => setRenaming(false)}
        />
      )}
      <MoreEntries
        label="操作"
        entries={[
          { kind: "item", label: "改名", onSelect: () => setRenaming(true) },
        ]}
      />
    </>
  );
}

// The menu hands focus back as it closes; the field an entry opened must keep it, or it blurs
// and closes before a key is typed.
it("leaves focus in the field an entry opened when a pointer closes the menu", async () => {
  const user = userEvent.setup();
  render(<RenameRow />);
  await user.click(screen.getByRole("button", { name: "操作" }));
  await user.click(await screen.findByRole("menuitem", { name: "改名" }));
  await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
  expect(screen.getByRole("textbox", { name: "任务标题" })).toHaveFocus();
});

it("leaves focus in the field an entry opened when the keyboard closes the menu", async () => {
  const user = userEvent.setup();
  render(<RenameRow />);
  screen.getByRole("button", { name: "操作" }).focus();
  await user.keyboard("{Enter}");
  await screen.findByRole("menuitem", { name: "改名" });
  await user.keyboard("{Enter}");
  await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
  expect(screen.getByRole("textbox", { name: "任务标题" })).toHaveFocus();
});
