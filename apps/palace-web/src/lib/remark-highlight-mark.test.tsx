import { render } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { expect, it } from "vitest";
import {
  MarkdownContent,
  type ImagePolicy,
  type MarkdownDialect,
} from "@/components/markdown-content";

const placeholder: ImagePolicy = { kind: "placeholder" };

// The rendered HTML of one Markdown source, in the given dialect.
function html(content: string, dialect: MarkdownDialect = "jotting") {
  const { container } = render(
    <MemoryRouter>
      <MarkdownContent
        content={content}
        images={placeholder}
        dialect={dialect}
      />
    </MemoryRouter>,
  );
  return container.querySelector(".markdown")!.innerHTML;
}

// specs/test-cases/web/echo/echo-page.md#jotting-bodies-must-highlight-double-equals-and-nothing-else-may
it.each([
  ["a ==highlight== here", "<p>a <mark>highlight</mark> here</p>"],
  // Full-width punctuation inside the pair and a CJK character after it, as with `**`.
  ["==重点。==后文", "<p><mark>重点。</mark>后文</p>"],
  [
    "一句==**加粗**和*斜体*==。",
    "<p>一句<mark><strong>加粗</strong>和<em>斜体</em></mark>。</p>",
  ],
  ["**==加粗里的重点==**", "<p><strong><mark>加粗里的重点</mark></strong></p>"],
  ["==跨\n行==", "<p><mark>跨\n行</mark></p>"],
  ["a==b==c", "<p>a<mark>b</mark>c</p>"],
])("marks %j", (source, expected) => {
  expect(html(source)).toBe(expected);
});

// specs/test-cases/web/echo/echo-page.md#jotting-bodies-must-highlight-double-equals-and-nothing-else-may
it.each([
  // An opener needs text after it, a closer text before it.
  ["a == b == c", "<p>a == b == c</p>"],
  ["== 空格 ==", "<p>== 空格 ==</p>"],
  // Only two `=` make a sequence.
  ["x===y===z", "<p>x===y===z</p>"],
  ["=单个=", "<p>=单个=</p>"],
  ["\\==转义==", "<p>==转义==</p>"],
  ["`==代码==`", "<p><code>==代码==</code></p>"],
  ["==没有收尾", "<p>==没有收尾</p>"],
])("leaves %j as text", (source, expected) => {
  expect(html(source)).toBe(expected);
});

// specs/test-cases/web/echo/echo-page.md#jotting-bodies-must-highlight-double-equals-and-nothing-else-may
it("keeps `==` as text outside jottings", () => {
  expect(html("==重点==", "common")).toBe("<p>==重点==</p>");
});
