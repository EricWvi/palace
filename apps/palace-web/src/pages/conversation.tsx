import { Fragment, useEffect } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  Link,
  useNavigate,
  useParams,
  useSearchParams,
} from "react-router-dom";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { conversationOptions, sources } from "@/lib/api";
import { resolvePaths } from "@/lib/conversation-tree";
import { backLabel, dayOf, dayParam, parseDay } from "@/lib/day";
import { PathFork } from "@/components/path-fork";
import { ErrorState } from "@/components/error-state";
import { ConversationMenu } from "@/components/conversation-menu";
import { MessageToc, messageAnchor } from "@/components/message-toc";

// 摘星's reading page for one conversation. `path` picks the branch; `date` says the reader came
// from that day in 时刻, which decides where "back" and deletion lead.
export function ConversationPage() {
  const { id } = useParams();
  const [params, setParams] = useSearchParams();
  const navigate = useNavigate();
  const detail = useQuery(conversationOptions(id!));
  const paths = detail.data ? resolvePaths(detail.data) : [];
  const wanted = params.get("path");
  const selected = paths.find(({ path }) => path.id === wanted) ?? paths[0];
  const from = parseDay(params.get("date"));
  const conversation = detail.data?.conversation;

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

  if (detail.isPending)
    return (
      <p role="status" className="quiet-note">
        正在加载对话…
      </p>
    );
  if (detail.isError)
    return (
      <ErrorState error={detail.error} retry={() => void detail.refetch()} />
    );
  if (!selected) return null;
  const source = sources[conversation!.source];
  return (
    <>
      <header className="conversation">
        {from && (
          <Link
            className="back"
            to={`/?date=${dayParam(from)}&moment=${selected.path.id}`}
          >
            {backLabel(from)}
          </Link>
        )}
        <div className="title-row reveals-more">
          <h1>{selected.path.title}</h1>
          <ConversationMenu
            key={selected.path.id}
            conversationId={id!}
            source={conversation!.source}
            path={selected.path}
            onImported={(result, occurredAt) =>
              show(result.path_id, dayParam(dayOf(occurredAt)))
            }
            onDeleted={() =>
              navigate(from ? `/?date=${dayParam(from)}` : "/", {
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
        </p>
      </header>
      <ol className="messages">
        {selected.messages.map((message, index) => (
          <Fragment key={message.id}>
            <li
              id={messageAnchor(message)}
              className={`message ${message.role}`}
            >
              <div className="who">
                {message.role === "user" ? "你" : source}
              </div>
              <div className="body">
                <Markdown
                  remarkPlugins={[remarkGfm]}
                  skipHtml
                  components={{
                    img: ({ alt }) => (
                      <span className="quiet">[图片：{alt || "未加载"}]</span>
                    ),
                    a: ({ children, href, title }) => (
                      <a
                        href={href}
                        title={title}
                        target="_blank"
                        rel="noreferrer"
                      >
                        {children}
                      </a>
                    ),
                  }}
                >
                  {message.content}
                </Markdown>
                {!message.content && <span className="quiet">（空消息）</span>}
              </div>
            </li>
            <PathFork
              paths={paths}
              selected={selected}
              after={index}
              onSelect={(pathId) => show(pathId)}
            />
          </Fragment>
        ))}
      </ol>
      <MessageToc messages={selected.messages} />
    </>
  );
}
