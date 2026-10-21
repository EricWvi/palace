import { Fragment, useEffect, useLayoutEffect, type ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  Link,
  useLocation,
  useNavigate,
  useParams,
  useSearchParams,
} from "react-router-dom";
import { conversationOptions, sources } from "@/lib/api";
import { resolvePaths } from "@/lib/conversation-tree";
import { backLabel, dayOf, dayParam, parseDay } from "@/lib/day";
import { PathFork } from "@/components/path-fork";
import { ErrorState } from "@/components/error-state";
import { ConversationMenu } from "@/components/conversation-menu";
import { MessageToc, messageAnchor } from "@/components/message-toc";
import {
  MarkdownContent,
  type ImagePolicy,
} from "@/components/markdown-content";
import {
  AnnotationFacts,
  useAnnotations,
  type Annotations,
} from "@/components/annotation-layer";
import { AnnotatedArea } from "@/components/annotated-area";
import type { Message } from "@/lib/api";

// Imported messages are third-party content: showing them must not fetch anything on the reader's behalf.
// Module-level, so the memoized content never sees a new policy object.
const imported: ImagePolicy = { kind: "placeholder" };

// 摘星's reading page for one conversation. `path` picks the branch; `date` says the reader came
// from that day in 时刻, which decides where "back" and deletion lead.

export function ConversationPage() {
  const { id } = useParams();
  const [params, setParams] = useSearchParams();
  const navigate = useNavigate();
  const detail = useQuery(conversationOptions(id!));
  const layer = useAnnotations({ kind: "conversation", id: id! });
  const paths = detail.data ? resolvePaths(detail.data) : [];
  const wanted = params.get("path");
  const selected = paths.find(({ path }) => path.id === wanted) ?? paths[0];
  const from = parseDay(params.get("date"));
  const conversation = detail.data?.conversation;
  // A row of 摘星's list asks to start at the top; the list keeps its own offset for going back.
  const { state } = useLocation();
  const top = (state as { top?: boolean } | null)?.top;
  useLayoutEffect(() => {
    if (top) window.scrollTo(0, 0);
  }, [top, id]);

  // Branch changes replace the entry: the reader is still on the same page, and back should lead
  // to the day, not through every branch looked at.
  function show(pathId: string, date = params.get("date")) {
    setParams(date ? { path: pathId, date } : { path: pathId }, {
      replace: true,
    });
  }

  // A path that no longer exists (deleted here, or a stale link) yields to the default one.
  useEffect(() => {
    if (selected && wanted && wanted !== selected.path.id)
      show(selected.path.id);
  });

  const title = selected?.path.title;
  useEffect(() => {
    if (title) document.title = title;
  }, [title]);

  // The way back needs only the address, so it shows before the conversation arrives. Until then
  // the moment to return to is the requested path, which links from 时刻 always name.
  const momentId = selected?.path.id ?? wanted;
  const back = from && (
    <Link
      className="back"
      to={`/?date=${dayParam(from)}${momentId ? `&moment=${momentId}` : ""}`}
    >
      {backLabel(from)}
    </Link>
  );

  // The title and facts are drawn as bars in the header's own box, so the page does not shift
  // when the conversation arrives; the bars mean nothing to a screen reader, the status does.
  if (detail.isPending)
    return (
      <header className="conversation">
        {back}
        <p role="status" className="sr-only">
          正在加载对话…
        </p>
        <div className="skeleton" aria-hidden="true">
          <span className="bar head" />
          <span className="bar meta" />
        </div>
      </header>
    );
  if (detail.isError)
    return (
      <ErrorState error={detail.error} retry={() => void detail.refetch()} />
    );
  if (!selected) return null;
  const source = sources[conversation!.source];
  return (
    <AnnotatedArea layer={layer}>
      <header className="conversation">
        {back}
        <div className="title-row reveals-more">
          <h1>{selected.path.title}</h1>
          <ConversationMenu
            key={selected.path.id}
            conversationId={id!}
            source={conversation!.source}
            path={selected.path}
            reading={layer.menuAction}
            onImported={(result, occurredAt) =>
              show(result.path_id, dayParam(dayOf(occurredAt)))
            }
            // Without a day the reader came from 摘星 or a bare link, so 摘星 takes them back.
            onDeleted={() =>
              navigate(from ? `/?date=${dayParam(from)}` : "/conversations", {
                replace: true,
              })
            }
          />
        </div>
        <p className="meta">
          <span>{source}</span>
          <span>{selected.messages.length} 条消息</span>
          <a
            href={selected.path.original_link}
            target="_blank"
            rel="noreferrer"
          >
            继续对话 ↗
          </a>
          <AnnotationFacts layer={layer} />
        </p>
      </header>
      <ol className="messages">
        {foldMessages(selected.messages, layer, (message, index) => (
          <Fragment key={message.id}>
            <li
              id={messageAnchor(message)}
              className={`message ${message.role}`}
            >
              <div className="who">
                {message.role === "user" ? "你" : source}
              </div>
              <div className="body">
                <MarkdownContent
                  content={message.content}
                  images={imported}
                  scope={message.id}
                  painting={layer.paintingFor(message.id)}
                />
                {!message.content && <span className="quiet">（空消息）</span>}
              </div>
            </li>
            {/* While only annotations show, the forks would hang between folded messages. */}
            {!layer.focus && (
              <PathFork
                paths={paths}
                selected={selected}
                after={index}
                onSelect={(pathId) => show(pathId)}
              />
            )}
          </Fragment>
        ))}
      </ol>
      <MessageToc messages={selected.messages} />
    </AnnotatedArea>
  );
}

// Folded runs of whole messages are counted under their own name, apart from each message's.
const messagesScope = "messages";

// Draws the messages; while only annotations show, each run of unannotated messages becomes one
// line that opens that run alone.
function foldMessages(
  messages: Message[],
  layer: Annotations,
  draw: (message: Message, index: number) => ReactNode,
) {
  if (!layer.focus) return messages.map(draw);
  const annotated = new Set(layer.list.map((item) => item.message_id));
  const out: ReactNode[] = [];
  let run: number[] = [];
  let number = 0;
  const flush = () => {
    if (!run.length) return;
    const current = number++;
    if (layer.opened(messagesScope).has(current))
      for (const index of run) out.push(draw(messages[index], index));
    else
      out.push(
        <li
          key={`fold-${current}`}
          className="fold-gap"
          title="展开"
          onClick={() => layer.open(messagesScope, current)}
        >
          省略 {run.length} 条消息
        </li>,
      );
    run = [];
  };
  messages.forEach((message, index) => {
    if (annotated.has(message.id)) {
      flush();
      out.push(draw(message, index));
    } else {
      run.push(index);
    }
  });
  flush();
  return out;
}
