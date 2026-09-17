import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it } from "vitest";
import {
  Dialog,
  DialogTrigger,
  DialogContent,
  DialogTitle,
  DialogDescription,
} from "./dialog";
import { Button } from "./button";
it("opens with an accessible title and returns focus after keyboard dismissal", async () => {
  const user = userEvent.setup();
  render(
    <Dialog>
      <DialogTrigger asChild>
        <Button>打开</Button>
      </DialogTrigger>
      <DialogContent>
        <DialogTitle>测试窗口</DialogTitle>
        <DialogDescription>辅助说明</DialogDescription>
      </DialogContent>
    </Dialog>,
  );
  await user.click(screen.getByRole("button", { name: "打开" }));
  expect(
    await screen.findByRole("dialog", { name: "测试窗口" }),
  ).toHaveAccessibleDescription("辅助说明");
  await user.keyboard("{Escape}");
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "打开" })).toHaveFocus();
});
