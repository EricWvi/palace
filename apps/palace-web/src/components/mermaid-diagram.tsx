import { cn } from "cn";
import { useEffect, useId, useState, useSyncExternalStore } from "react";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";

const darkScheme = "(prefers-color-scheme: dark)";

// The OS colour scheme, so a diagram redraws in the other palette when it flips.
function useDarkScheme() {
  return useSyncExternalStore(
    (changed) => {
      const query = window.matchMedia?.(darkScheme);
      query?.addEventListener("change", changed);
      return () => query?.removeEventListener("change", changed);
    },
    () => window.matchMedia?.(darkScheme).matches ?? false,
  );
}

// Mermaid's own themes are cool greys and blues that clash with the warm page, so its neutral
// `base` theme is fed the page's tokens, read at draw time so they follow whichever theme is on.
function themeVariables(dark: boolean) {
  const style = getComputedStyle(document.documentElement);
  const token = (name: string) => style.getPropertyValue(name).trim();
  return {
    darkMode: dark,
    background: token("--bg"),
    primaryColor: token("--bg-sunken"),
    primaryTextColor: token("--text"),
    primaryBorderColor: token("--border"),
    secondaryColor: token("--bg-sunken"),
    tertiaryColor: token("--bg"),
    lineColor: token("--text-3"),
    textColor: token("--text"),
    fontFamily: token("--font-ui"),
    // A drawn shadow is decoration the text-first page does without.
    dropShadow: "none",
  };
}

type Drawn =
  | { kind: "drawing" }
  | { kind: "drawn"; inline: string; full: string }
  | { kind: "failed" };

// One fenced `mermaid` block drawn as a diagram. Inline it fits the column as an overview; a
// click opens it at natural size, where a wide chart can be scrolled instead of squinted at.
// Mermaid is loaded on the first diagram only, since it outweighs the rest of the page.
export function MermaidDiagram({
  code,
  className,
}: {
  code: string;
  // Carries 只看标注's folding, as on any other block.
  className?: string;
}) {
  const dark = useDarkScheme();
  const id = `mermaid-${useId().replace(/[^a-zA-Z0-9]/g, "")}`;
  const [drawn, setDrawn] = useState<Drawn>({ kind: "drawing" });
  const [open, setOpen] = useState(false);

  useEffect(() => {
    let live = true;
    void (async () => {
      try {
        const { default: mermaid } = await import("mermaid");
        // `strict` escapes labels and disables click handlers: conversations are third-party
        // text.
        mermaid.initialize({
          startOnLoad: false,
          securityLevel: "strict",
          // A failed diagram falls back to its source here; mermaid's own error picture would
          // otherwise be left at the end of the page.
          suppressErrorRendering: true,
          theme: "base",
          themeVariables: themeVariables(dark),
        });
        const { svg } = await mermaid.render(id, code);
        // Mermaid scopes the diagram's styles by its root id, so the enlarged copy needs its
        // own; it also trades the fit-to-column width for the size the viewBox was laid out at.
        const [, , width, height] =
          /viewBox="([^"]+)"/.exec(svg)?.[1].split(/\s+/) ?? [];
        const full = svg
          .replaceAll(id, `${id}-full`)
          .replace('width="100%"', `width="${width}" height="${height}"`)
          .replace(/max-width:\s*[\d.]+px;?/, "");
        if (live) setDrawn({ kind: "drawn", inline: svg, full });
      } catch {
        if (live) setDrawn({ kind: "failed" });
      }
    })();
    return () => {
      live = false;
    };
  }, [code, dark, id]);

  // A diagram that does not parse still shows what was written.
  if (drawn.kind === "failed")
    return (
      <pre className={className} data-annotation-skip="">
        <code>{code}</code>
      </pre>
    );
  if (drawn.kind === "drawing")
    return (
      <div
        className={cn("mermaid-diagram", className)}
        data-annotation-skip=""
      />
    );
  return (
    <>
      <button
        type="button"
        className={cn("mermaid-diagram", className)}
        data-annotation-skip=""
        aria-label="放大图表"
        onClick={() => setOpen(true)}
        dangerouslySetInnerHTML={{ __html: drawn.inline }}
      />
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="mermaid-full" aria-describedby={undefined}>
          <DialogTitle className="sr-only">图表</DialogTitle>
          <div dangerouslySetInnerHTML={{ __html: drawn.full }} />
        </DialogContent>
      </Dialog>
    </>
  );
}
