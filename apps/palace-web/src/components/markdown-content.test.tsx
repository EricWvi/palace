import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes, useLocation } from "react-router-dom";
import { expect, it, vi } from "vitest";
import { MarkdownContent, type ImagePolicy } from "./markdown-content";

const placeholder: ImagePolicy = { kind: "placeholder" };
// Shows the current path, so an in-app link can be seen to navigate without leaving the page.
function Where() {
  return <output aria-label="位置">{useLocation().pathname}</output>;
}
function show(content: string, images: ImagePolicy = placeholder) {
  return render(
    <MemoryRouter initialEntries={["/notes/1"]}>
      <Routes>
        <Route
          path="*"
          element={
            <>
              <MarkdownContent content={content} images={images} />
              <Where />
            </>
          }
        />
      </Routes>
    </MemoryRouter>,
  );
}

const fence = (language: string, code: string) =>
  `\`\`\`${language}\n${code}\n\`\`\``;
const blocks = [
  ["js", 'const answer = "42"; // why'],
  ["tsx", "function App() { return <div /> }"],
  ["html", '<a href="/">home</a>'],
  ["", "SELECT * FROM logs"],
  ["text", "function looks() { like code }"],
  ["brainfuck", "++[>+<-]"],
] as const;

// Core test case: `specs/test-cases/server/conversation/markdown-content.md#only-code-blocks-labeled-with-a-known-language-may-be-highlighted`
it("highlights only blocks labeled with a known language and keeps every text as written", () => {
  const { container } = show(
    [
      ...blocks.map(([language, code]) => fence(language, code)),
      "Inline `const x = 1` and <b>raw</b> HTML.",
    ].join("\n\n"),
  );
  const codes = [...container.querySelectorAll("pre > code")];
  expect(
    codes.map((code) => [
      code.textContent,
      code.querySelector("[class^='hljs-']") !== null,
    ]),
  ).toEqual(
    blocks.map(([language, code]) => [
      `${code}\n`,
      ["js", "tsx", "html"].includes(language),
    ]),
  );
  // Each named class lands where the grammar puts it, and the first render already carries it.
  expect(
    [...codes[0].querySelectorAll("[class^='hljs-']")].map((span) => [
      span.className,
      span.textContent,
    ]),
  ).toEqual([
    ["hljs-keyword", "const"],
    ["hljs-string", '"42"'],
    ["hljs-comment", "// why"],
  ]);
  const inline = container.querySelector(":not(pre) > code")!;
  expect([inline.textContent, inline.children.length]).toEqual([
    "const x = 1",
    0,
  ]);
  expect(container.querySelector("b")).toBeNull();
});

// Core test case: `specs/test-cases/server/conversation/markdown-content.md#the-copy-button-must-copy-the-original-code-and-stay-out-of-sight-until-wanted`
it("copies the fenced original, says so, and falls back after a moment", async () => {
  const user = userEvent.setup();
  show(
    `${fence("js", "const a = 1;\n\nconst b = 2;")}\n\n${fence("", "plain")}`,
  );
  const writeText = vi.spyOn(navigator.clipboard, "writeText");
  const [first, second] = screen.getAllByRole("button", { name: "复制代码" });
  await user.click(first);
  expect(writeText).toHaveBeenLastCalledWith("const a = 1;\n\nconst b = 2;");
  expect(first).toHaveTextContent("已复制");
  const status = within(first.parentElement!).getByRole("status");
  expect(status).toHaveTextContent("已复制");
  await waitFor(() => expect(first).toHaveTextContent(/^复制$/), {
    timeout: 2500,
  });
  expect(status).toBeEmptyDOMElement();
  writeText.mockRejectedValueOnce(new Error("denied"));
  await user.click(second);
  expect(writeText).toHaveBeenLastCalledWith("plain");
  expect(second).toHaveTextContent("复制失败");
  await waitFor(() => expect(second).toHaveTextContent(/^复制$/), {
    timeout: 2500,
  });
});

// Core test cases:
// - `specs/test-cases/server/conversation/markdown-content.md#images-must-load-only-from-addresses-the-caller-resolves`
// - `specs/test-cases/web/stars/articles.md#article-page-must-load-only-its-own-palace-media`
it("loads images only where the caller resolves them, lazily and without a referrer", () => {
  const content =
    "![猫](/api/media/cat) ![外站](https://tracker.example/pixel.png) ![](/api/media/dog)";
  const { container, unmount } = show(content);
  expect(container.querySelector("img")).toBeNull();
  expect(
    [...container.querySelectorAll(".quiet")].map((el) => el.textContent),
  ).toEqual(["[图片：猫]", "[图片：外站]", "[图片：未加载]"]);
  unmount();
  const resolve = vi.fn((src: string) =>
    src.startsWith("/api/media/") ? { kind: "image" as const, src } : null,
  );
  const loaded = show(content, { kind: "load", resolve });
  expect(
    [...loaded.container.querySelectorAll("img")].map((img) => [
      img.getAttribute("src"),
      img.getAttribute("alt"),
      img.getAttribute("loading"),
      img.getAttribute("referrerpolicy"),
    ]),
  ).toEqual([
    ["/api/media/cat", "猫", "lazy", "no-referrer"],
    ["/api/media/dog", "", "lazy", "no-referrer"],
  ]);
  expect(
    [...loaded.container.querySelectorAll(".quiet")].map(
      (el) => el.textContent,
    ),
  ).toEqual(["[图片：外站]"]);
});

// Core test case: `specs/test-cases/web/stars/articles.md#article-page-must-load-only-its-own-palace-media`
it("plays what the caller resolves as a video, loading only its metadata", () => {
  const { container } = show(
    "![演示](/api/media/demo) ![封面](/api/media/cover)",
    {
      kind: "load",
      resolve: (src) => ({
        kind: src.endsWith("demo") ? "video" : "image",
        src,
      }),
    },
  );
  expect(
    [...container.querySelectorAll("video")].map((video) => [
      video.getAttribute("src"),
      video.getAttribute("aria-label"),
      video.hasAttribute("controls"),
      video.getAttribute("preload"),
    ]),
  ).toEqual([["/api/media/demo", "演示", true, "metadata"]]);
  expect(
    [...container.querySelectorAll("img")].map((img) =>
      img.getAttribute("src"),
    ),
  ).toEqual(["/api/media/cover"]);
});

// Core test case: `specs/test-cases/server/conversation/markdown-content.md#in-app-links-must-stay-in-the-page-and-others-must-open-a-new-window`
it("follows in-app paths in place and opens other links in a new window", async () => {
  const user = userEvent.setup();
  show(
    "[对话](/conversations/tree) [外站](https://example.com) [协议相对](//example.com)",
  );
  const external = screen.getByRole("link", { name: "外站" });
  const relative = screen.getByRole("link", { name: "协议相对" });
  expect(
    [external, relative].map((link) => [
      link.getAttribute("target"),
      link.getAttribute("rel"),
    ]),
  ).toEqual([
    ["_blank", "noreferrer"],
    ["_blank", "noreferrer"],
  ]);
  const inApp = screen.getByRole("link", { name: "对话" });
  expect(inApp).not.toHaveAttribute("target");
  await user.click(inApp);
  expect(screen.getByRole("status", { name: "位置" })).toHaveTextContent(
    "/conversations/tree",
  );
});
