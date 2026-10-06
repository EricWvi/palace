import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { expect, it } from "vitest";
import { FileDrop } from "./file-drop";
import { FieldHelp } from "./field-help";

function Field() {
  const [file, setFile] = useState<File | null>(null);
  return (
    <>
      <label htmlFor="history">对话文件</label>
      <FileDrop id="history" accept=".json" file={file} onFile={setFile} />
    </>
  );
}
const chat = () => new File(["[]"], "chat.json", { type: "application/json" });

it("takes a file either browsed through its label or dropped onto the box", async () => {
  const user = userEvent.setup();
  const { unmount } = render(<Field />);
  expect(screen.getByText("选择 JSON 文件")).toBeInTheDocument();
  await user.upload(screen.getByLabelText("对话文件"), chat());
  expect(screen.getByText("chat.json")).toBeInTheDocument();
  unmount();

  render(<Field />);
  const box = screen.getByText("或将文件拖到这里").closest("label")!;
  fireEvent.dragOver(box);
  expect(box).toHaveAttribute("data-over");
  fireEvent.drop(box, { dataTransfer: { files: [chat()] } });
  expect(box).not.toHaveAttribute("data-over");
  expect(screen.getByText("chat.json")).toBeInTheDocument();
});

it("shows a field's rules when its help mark takes keyboard focus", async () => {
  const user = userEvent.setup();
  render(
    <FieldHelp label="Session ID">同一来源的 Session ID 不可重复。</FieldHelp>,
  );
  await user.tab();
  expect(screen.getByRole("button", { name: "Session ID说明" })).toHaveFocus();
  expect(await screen.findByRole("tooltip")).toHaveTextContent(
    "同一来源的 Session ID 不可重复。",
  );
});
