import type { HeadingLevel, TocItem } from "./reading-toc";

// Read the rendered headings so fenced examples, Markdown marks and Setext syntax need no
// second parser. Positional anchors distinguish repeated and empty headings without slug collisions.
export function noteHeadings(root: HTMLElement): TocItem[] {
  return Array.from(
    root.querySelectorAll("h1, h2, h3, h4, h5, h6"),
    (heading, index) => {
      const id = `note-heading-${index}`;
      heading.id = id;
      return {
        id,
        label: heading.textContent?.trim() || "（空标题）",
        kind: "heading",
        level: Number(heading.tagName.slice(1)) as HeadingLevel,
      };
    },
  );
}
