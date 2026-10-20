import { memo, useEffect, useRef, useState, type ComponentProps } from "react";
import Markdown from "react-markdown";
import { Link } from "react-router-dom";
import rehypeHighlight, { type Options } from "rehype-highlight";
import remarkGfm from "remark-gfm";

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

// Shows Markdown the same way everywhere: GFM, no raw HTML, highlighted fenced code with a copy
// button, external links in a new window and in-app paths through the router.
export const MarkdownContent = memo(function MarkdownContent({
  content,
  images,
}: {
  content: string;
  // Pass a stable value (a module constant or a memoized object) so memoization holds.
  images: ImagePolicy;
}) {
  return (
    <div className="markdown">
      <Markdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[[rehypeHighlight, highlight]]}
        skipHtml
        components={{
          img: ({ src, alt }) => {
            const resolved =
              images.kind === "load" && typeof src === "string"
                ? images.resolve(src)
                : null;
            if (resolved === null)
              return <span className="quiet">[图片：{alt || "未加载"}]</span>;
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
          pre: ({ children }) => <CodeBlock>{children}</CodeBlock>,
        }}
      >
        {content}
      </Markdown>
    </div>
  );
});

const labels = { idle: "复制", copied: "已复制", failed: "复制失败" } as const;

// The button sits outside the scrolling <pre>, so it stays in the corner while wide code scrolls.
function CodeBlock({ children }: ComponentProps<"pre">) {
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
    <div className="code-block">
      <pre ref={code}>{children}</pre>
      <button
        type="button"
        className="copy-code"
        aria-label="复制代码"
        onClick={copy}
      >
        {labels[state]}
      </button>
      <span role="status" className="sr-only">
        {state === "idle" ? "" : labels[state]}
      </span>
    </div>
  );
}
