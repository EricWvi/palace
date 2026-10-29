import "katex/dist/katex.min.css";
import type { Element, ElementContent, Root } from "hast";
import { memo, useEffect, useRef, useState, type ComponentProps } from "react";
import Markdown, { type Options as MarkdownOptions } from "react-markdown";
import { Link } from "react-router-dom";
import rehypeHighlight, { type Options } from "rehype-highlight";
import rehypeKatex from "rehype-katex";
import remarkCjkFriendly from "remark-cjk-friendly";
import remarkCjkFriendlyGfmStrikethrough from "remark-cjk-friendly-gfm-strikethrough";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import { rehypeAnnotations, type Painting } from "@/lib/annotation-paint";
import { remarkDollarMath } from "@/lib/remark-dollar-math";
import { MermaidDiagram } from "./mermaid-diagram";

// Where images may be loaded from is the one rule that differs between callers: imported
// conversations are third-party content and must not make the reader's browser fetch anything,
// while notes and articles show images and videos the owner keeps.
export type ImagePolicy =
  | { kind: "placeholder" }
  // `resolve` maps the address written in the Markdown to what to load; null keeps a placeholder.
  | { kind: "load"; resolve: (src: string) => EmbeddedMedia | null };

// Markdown has one embed syntax, `![alt](src)`; only the caller knows whether src is a video.
export type EmbeddedMedia = { kind: "image" | "video"; src: string };

// Only a language the source wrote down is trusted: guessing turns logs and command output into
// confidently wrong colours, and trying every grammar on every block grows with the content.
const highlight: Options = {
  detect: false,
  plainText: ["text", "txt", "plaintext"],
};

const katexClasses = new Set(["katex", "katex-display", "katex-error"]);

function classes(element: Element) {
  const value = element.properties.className;
  return Array.isArray(value) ? value.map(String) : [];
}

function codeOf(pre: Element) {
  return pre.children.find(
    (child): child is Element =>
      child.type === "element" && child.tagName === "code",
  );
}

function isMermaid(pre: Element) {
  const code = codeOf(pre);
  return code !== undefined && classes(code).includes("language-mermaid");
}

function textOf(node: ElementContent): string {
  if (node.type === "text") return node.value;
  return node.type === "element" ? node.children.map(textOf).join("") : "";
}

// Flags rendered formulas and mermaid blocks `data-annotation-skip`. Each is one picture: its
// glyphs or diagram source are no text a reader selects a quote in, and painting a mark inside
// KaTeX's layout spans would break the formula. Both sides of the annotation text skip the flag.
function rehypeAtomicBlocks() {
  const walk = (parent: Root | Element) => {
    for (const child of parent.children) {
      if (child.type !== "element") continue;
      const atomic =
        classes(child).some((name) => katexClasses.has(name)) ||
        (child.tagName === "pre" && isMermaid(child));
      if (atomic) child.properties.dataAnnotationSkip = "";
      else walk(child);
    }
  };
  return walk;
}

// CommonMark will not close `**加粗。**后文` because the run sits between full-width punctuation
// and a CJK character; the CJK-friendly extensions relax that rule for emphasis and `~~`.
// remark-math pairs any two single `$`, so prose with a lone `$` or two amounts turns into a
// formula; single-dollar formulas use the stricter rules of `remarkDollarMath` instead.
const remarkPlugins: NonNullable<MarkdownOptions["remarkPlugins"]> = [
  remarkGfm,
  remarkCjkFriendly,
  remarkCjkFriendlyGfmStrikethrough,
  [remarkMath, { singleDollarTextMath: false }],
  remarkDollarMath,
];

// KaTeX runs first so highlighting never sees the `language-math` code it replaces.
const rehypePlugins: NonNullable<MarkdownOptions["rehypePlugins"]> = [
  rehypeKatex,
  [rehypeHighlight, highlight],
  rehypeAtomicBlocks,
];

// Shows Markdown the same way everywhere: GFM, `$` and `$$` formulas, mermaid diagrams, no raw
// HTML, highlighted fenced code with a copy button, external links in a new window and in-app
// paths through the router. Text this component adds itself (the copy button, image
// placeholders) and formulas and diagrams carry `data-annotation-skip`, so they are never part
// of the text annotations are anchored in.
export const MarkdownContent = memo(function MarkdownContent({
  content,
  images,
  scope,
  painting,
}: {
  content: string;
  // Pass a stable value (a module constant or a memoized object) so memoization holds.
  images: ImagePolicy;
  // Names the annotation scope this text is, for turning a selection in it into an anchor.
  scope?: string;
  // The annotations to draw and the folding of 只看标注; stable, like `images`.
  painting?: Painting;
}) {
  return (
    <div className="markdown" data-annotation-scope={scope}>
      <Markdown
        remarkPlugins={remarkPlugins}
        rehypePlugins={
          painting
            ? [...rehypePlugins, [rehypeAnnotations, painting]]
            : rehypePlugins
        }
        skipHtml
        components={{
          img: ({ src, alt }) => {
            const resolved =
              images.kind === "load" && typeof src === "string"
                ? images.resolve(src)
                : null;
            if (resolved === null)
              return (
                <span className="quiet" data-annotation-skip="">
                  [图片：{alt || "未加载"}]
                </span>
              );
            // Only the metadata loads up front; the video itself waits for the reader to play it.
            return resolved.kind === "video" ? (
              <video
                src={resolved.src}
                aria-label={alt || undefined}
                controls
                preload="metadata"
              />
            ) : (
              <img
                src={resolved.src}
                alt={alt ?? ""}
                loading="lazy"
                referrerPolicy="no-referrer"
              />
            );
          },
          a: ({ children, href, title }) =>
            // A single leading slash is a page of this app; `//host` is another site.
            href?.startsWith("/") && !href.startsWith("//") ? (
              <Link to={href} title={title}>
                {children}
              </Link>
            ) : (
              <a href={href} title={title} target="_blank" rel="noreferrer">
                {children}
              </a>
            ),
          // The class carries 只看标注's folding onto the block the reader sees.
          pre: ({ children, className, node }) => {
            if (node && isMermaid(node))
              return (
                <MermaidDiagram
                  code={textOf(codeOf(node)!).replace(/\n$/, "")}
                  className={className}
                />
              );
            return <CodeBlock className={className}>{children}</CodeBlock>;
          },
        }}
      >
        {content}
      </Markdown>
    </div>
  );
});

const labels = { idle: "复制", copied: "已复制", failed: "复制失败" } as const;

// The button sits outside the scrolling <pre>, so it stays in the corner while wide code scrolls.
function CodeBlock({ children, className }: ComponentProps<"pre">) {
  const code = useRef<HTMLPreElement>(null);
  const [state, setState] = useState<keyof typeof labels>("idle");
  useEffect(() => {
    if (state === "idle") return;
    const timer = setTimeout(() => setState("idle"), 1500);
    return () => clearTimeout(timer);
  }, [state]);
  async function copy() {
    // Highlighting only wraps text in spans, so the rendered text is the fenced original; Markdown
    // parsing leaves one closing newline that was never part of the code.
    const text = (code.current?.textContent ?? "").replace(/\n$/, "");
    try {
      await navigator.clipboard.writeText(text);
      setState("copied");
    } catch {
      setState("failed");
    }
  }
  return (
    <div className={className ? `code-block ${className}` : "code-block"}>
      <pre ref={code}>{children}</pre>
      <button
        type="button"
        className="copy-code"
        data-annotation-skip=""
        aria-label="复制代码"
        onClick={copy}
      >
        {labels[state]}
      </button>
      <span role="status" className="sr-only" data-annotation-skip="">
        {state === "idle" ? "" : labels[state]}
      </span>
    </div>
  );
}
