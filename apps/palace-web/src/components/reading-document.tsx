import { useCallback, useState, type ReactNode } from "react";
import Markdown from "react-markdown";
import { noteHeadings } from "./note-toc";
import { ReadingToc, type TocItem } from "./reading-toc";
import { MarkdownContent, type ImagePolicy } from "./markdown-content";
import type { Painting } from "@/lib/annotation-paint";

// A level-1 ATX heading on the body's first line, with its optional closing `#`s.
const leadingHeading = /^\s*# +(.*?\S)(?: +#+)? *(?:\n|$)/;

// Inline marks a heading may carry; anything else (links, images) keeps only its text, since
// the page title is not a place to navigate from.
const inline = ["strong", "em", "code", "del"];

// One Markdown document read in 摘星, laid out the same whatever it is: a note or an article.
// The page above it only decides what the header's line of facts says and whether a menu sits
// beside the title.
export function ReadingDocument({
  id,
  title,
  content,
  images,
  meta,
  back,
  menu,
  tocLabel,
  scope,
  painting,
}: {
  // Identifies the document, so its table of contents starts afresh when another one opens.
  id: string;
  title: string;
  content: string;
  // Pass a stable value (a module constant or a memoized object) so the body stays memoized.
  images: ImagePolicy;
  meta: ReactNode;
  back?: ReactNode;
  menu?: ReactNode;
  tocLabel: string;
  // An annotated document names its body's scope and passes what to draw on it; see
  // MarkdownContent. The body excludes a leading heading promoted into the header.
  scope?: string;
  painting?: Painting;
}) {
  const [headings, setHeadings] = useState<TocItem[]>([]);
  // Recollect after React commits a changed document, before it is painted. The rendered DOM is
  // the source of truth, including a leading Markdown h1 promoted into the page header.
  const collectHeadings = useCallback(
    (root: HTMLDivElement | null) => {
      if (root) setHeadings(noteHeadings(root));
    },
    // The callback changes with the document, so React calls it again after each new one.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [id, title, content],
  );
  // Most documents repeat their title as the body's first heading. Showing both stacks two
  // titles, so the body's heading takes the header's place and the facts hang below it.
  const heading = leadingHeading.exec(content);
  const body = heading ? content.slice(heading[0].length) : content;
  const h1 = (
    <h1>
      {heading ? (
        <Markdown allowedElements={inline} unwrapDisallowed skipHtml>
          {heading[1]}
        </Markdown>
      ) : (
        title
      )}
    </h1>
  );
  return (
    <>
      <div ref={collectHeadings}>
        <header className="note">
          {back}
          {menu ? (
            <div className="title-row reveals-more">
              {h1}
              {menu}
            </div>
          ) : (
            h1
          )}
          <p className="meta">{meta}</p>
        </header>
        <div className="note-body">
          <MarkdownContent
            content={body}
            images={images}
            scope={scope}
            painting={painting}
          />
        </div>
      </div>
      <ReadingToc key={id} items={headings} label={tocLabel} />
    </>
  );
}

// The header's shape while the document loads, so the page does not shift when it arrives.
export function ReadingSkeleton({
  label,
  back,
}: {
  label: string;
  back?: ReactNode;
}) {
  return (
    <header className="note">
      {back}
      <p role="status" className="sr-only">
        {label}
      </p>
      <div className="skeleton" aria-hidden="true">
        <span className="bar head" />
        <span className="bar meta" />
      </div>
    </header>
  );
}

// `YYYY.MM.DD` in the reader's time zone.
export function fullDate(date: Date) {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${date.getFullYear()}.${pad(date.getMonth() + 1)}.${pad(date.getDate())}`;
}
