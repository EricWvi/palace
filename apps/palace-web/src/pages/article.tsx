import { useEffect, useLayoutEffect, useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  Link,
  useLocation,
  useNavigate,
  useParams,
  useSearchParams,
} from "react-router-dom";
import { ApiError, articleOptions, type Article } from "@/lib/api";
import { backLabel, dayParam, parseDay } from "@/lib/day";
import { ErrorState } from "@/components/error-state";
import { ArticleMenu } from "@/components/article-menu";
import { type ImagePolicy } from "@/components/markdown-content";
import {
  AnnotationFacts,
  articleScope,
  useAnnotations,
} from "@/components/annotation-layer";
import { AnnotatedArea } from "@/components/annotated-area";
import {
  ReadingDocument,
  ReadingSkeleton,
  fullDate,
} from "@/components/reading-document";

const media = /^\/api\/media\/([0-9a-f-]{36})$/;

// An article shows only the files imported with it, each as the kind its type says. Any other
// address, the original site's included, stays a placeholder: reading must not reach outside.
function ownMedia(article: Article): ImagePolicy {
  const types = new Map(
    article.media.map((file) => [file.id, file.content_type]),
  );
  return {
    kind: "load",
    resolve: (src) => {
      const type = types.get(media.exec(src)?.[1] ?? "");
      if (type === undefined) return null;
      return { kind: type.startsWith("video/") ? "video" : "image", src };
    },
  };
}

// 摘星's reading page for one article: laid out as a note, with the article's author, publish
// date and original address under the title, and a menu to correct those or delete it. `date`
// says the reader came from that day in 时刻, which decides where "back" and deletion lead.
export function ArticlePage() {
  const { id } = useParams();
  const [params] = useSearchParams();
  const navigate = useNavigate();
  const article = useQuery(articleOptions(id!));
  const layer = useAnnotations({ kind: "article", id: id! });
  const from = parseDay(params.get("date"));
  const { state } = useLocation();
  const top = (state as { top?: boolean } | null)?.top;
  useLayoutEffect(() => {
    if (top) window.scrollTo(0, 0);
  }, [top, id]);
  const title = article.data?.title;
  useEffect(() => {
    if (title) document.title = title;
  }, [title]);
  // One policy per loaded article, so the memoized body re-renders only when the article does.
  const images = useMemo(
    () => (article.data ? ownMedia(article.data) : null),
    [article.data],
  );

  const back = from && (
    <Link className="back" to={`/?date=${dayParam(from)}&moment=${id}`}>
      {backLabel(from)}
    </Link>
  );
  if (article.isPending)
    return <ReadingSkeleton label="正在加载文章…" back={back} />;
  if (article.isError)
    return (
      <ErrorState
        error={
          article.error instanceof ApiError && article.error.status === 404
            ? new ApiError(404, "找不到这篇文章。")
            : article.error
        }
        retry={() => void article.refetch()}
      />
    );
  const published = new Date(article.data.published_at);
  return (
    <AnnotatedArea layer={layer}>
      <ReadingDocument
        id={article.data.id}
        title={article.data.title}
        content={article.data.content}
        images={images!}
        scope={articleScope}
        painting={layer.paintingFor(articleScope)}
        back={back}
        menu={
          <ArticleMenu
            key={article.data.id}
            article={article.data}
            reading={layer.menuAction}
            // Without a day the reader came from 摘星 or a bare link, so 摘星 takes them back.
            onDeleted={() =>
              navigate(from ? `/?date=${dayParam(from)}` : "/articles", {
                replace: true,
              })
            }
          />
        }
        meta={
          <>
            {article.data.author && <span>{article.data.author}</span>}
            <time dateTime={published.toISOString()}>
              {fullDate(published)}
            </time>
            <a href={article.data.url} target="_blank" rel="noreferrer">
              原文 ↗
            </a>
            <AnnotationFacts layer={layer} />
          </>
        }
        tocLabel="文章目录"
      />
    </AnnotatedArea>
  );
}
