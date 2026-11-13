import { useRef, useState } from "react";
import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import { requestPath, testRequest } from "@/lib/test-request";
import { useImagePaste } from "./use-image-paste";

// A field as the composer and the editor keep theirs: controlled text, the hook's paste, and
// whether saving is held back.
function Field({ initial }: { initial: string }) {
  const field = useRef<HTMLTextAreaElement>(null);
  const [text, setText] = useState(initial);
  const paste = useImagePaste(field, setText);
  return (
    <>
      <textarea
        ref={field}
        aria-label="正文"
        value={text}
        onChange={(event) => setText(event.target.value)}
        onPaste={paste.onPaste}
      />
      <output aria-label="状态">
        {paste.uploading ? "上传中" : (paste.error ?? "")}
      </output>
    </>
  );
}

// Serves uploads one by one: each answers only when the test resolves it, with its id or a
// failure, and records the part it was sent.
function serveUploads() {
  const sent: { name: string; type: string }[] = [];
  const answers: ((answer: Response) => void)[] = [];
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
    expect(requestPath(input)).toBe("/api/media");
    const file = (await testRequest(input).formData()).get("file") as File;
    sent.push({ name: file.name, type: file.type });
    return new Promise<Response>((resolve) => answers.push(resolve));
  });
  const answer = async (index: number, response: Response) => {
    await vi.waitFor(() => expect(answers[index]).toBeDefined());
    await act(async () => answers[index](response));
  };
  return { sent, answer };
}

const stored = (id: string) =>
  Response.json(
    {
      id,
      content_type: "image/png",
      byte_size: 1,
      file_name: "image.png",
      created_at: 0,
    },
    { status: 201 },
  );

function paste(field: HTMLElement, files: File[]) {
  fireEvent.paste(field, { clipboardData: { files } });
}

afterEach(() => vi.restoreAllMocks());

// specs/test-cases/web/echo/echo-page.md#a-pasted-picture-must-become-palace-media-in-place
it("writes a pasted picture in place once stored, while typing goes on", async () => {
  const { sent, answer } = serveUploads();
  const user = userEvent.setup();
  render(<Field initial="前文后文" />);
  const field = screen.getByRole<HTMLTextAreaElement>("textbox");
  field.focus();
  field.setSelectionRange(2, 2);
  paste(field, [new File(["x"], "image.png", { type: "image/png" })]);
  expect(field).toHaveValue("前文![上传中 image.png…](uploading-1)后文");
  expect(screen.getByRole("status")).toHaveTextContent("上传中");

  // Typing goes on while the picture uploads, and the caret stays with the words typed.
  await user.keyboard("，");
  await answer(0, stored("01a169c8-2109-774d-a490-e8537c284595"));
  expect(field).toHaveValue(
    "前文![image.png](/api/media/01a169c8-2109-774d-a490-e8537c284595)，后文",
  );
  await user.keyboard("。");
  expect(field).toHaveValue(
    "前文![image.png](/api/media/01a169c8-2109-774d-a490-e8537c284595)，。后文",
  );
  expect(sent).toEqual([{ name: "image.png", type: "image/png" }]);
  expect(screen.getByRole("status")).toHaveTextContent("");
});

it("uploads each pasted picture on its own line and drops one that fails", async () => {
  const { sent, answer } = serveUploads();
  render(<Field initial="" />);
  const field = screen.getByRole<HTMLTextAreaElement>("textbox");
  field.focus();
  paste(field, [
    new File(["x"], "a.jpg", { type: "image/jpeg" }),
    new File(["x"], "notes.txt", { type: "text/plain" }),
    new File(["x"], "b.png", { type: "image/png" }),
  ]);
  await answer(1, Response.json({ error: "internal" }, { status: 500 }));
  expect(screen.getByRole("status")).toHaveTextContent("上传中");
  await answer(0, stored("0192"));
  expect(field.value).toMatch(/^!\[a\.jpg\]\(\/api\/media\/0192\)\n$/);
  expect(screen.getByRole("status")).toHaveTextContent(
    "图片上传失败：服务暂时不可用，请稍后重试。",
  );
  expect(sent).toEqual([
    { name: "a.jpg", type: "image/jpeg" },
    { name: "b.png", type: "image/png" },
  ]);
});

it("leaves a paste without pictures to the field", () => {
  const fetch = vi.spyOn(globalThis, "fetch");
  render(<Field initial="" />);
  const field = screen.getByRole("textbox");
  const event = new Event("paste", { bubbles: true, cancelable: true });
  Object.assign(event, { clipboardData: { files: [] } });
  field.dispatchEvent(event);
  expect(event.defaultPrevented).toBe(false);
  expect(fetch).not.toHaveBeenCalled();
});
