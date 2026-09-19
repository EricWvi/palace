import { useQuery } from "@tanstack/react-query";
import { Link, useParams, useSearchParams } from "react-router-dom";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { ArrowLeft, ExternalLink, Sparkles } from "lucide-react";
import { request, sources, type Detail } from "@/lib/api";
import { resolvePaths } from "@/lib/conversation-tree";
import { ErrorState } from "@/components/error-state";
export function ConversationPage() {
  const { id } = useParams();
  const [params, setParams] = useSearchParams();
  const detail = useQuery({
    queryKey: ["conversation", id],
    queryFn: () =>
      request<Detail>(`/api/conversations/${encodeURIComponent(id!)}`),
  });
  const paths = detail.data ? resolvePaths(detail.data) : [];
  const selected =
    paths.find(({ path }) => path.id === params.get("path")) ?? paths[0];
  const conversation = detail.data?.conversation;
  return (
    <>
      <header className="topbar">
        <Link to="/" className="back-link">
          <ArrowLeft size={16} />
          返回会话收藏
        </Link>
      </header>
      {detail.isPending ? (
        <p role="status" className="empty">
          正在加载会话…
        </p>
      ) : detail.isError ? (
        <ErrorState error={detail.error} retry={() => void detail.refetch()} />
      ) : (
        <section className="chat">
          <header className="chat-heading">
            <p className="eyebrow">
              {sources[conversation!.source]} · 会话存档
            </p>
            <h1>{conversation!.title}</h1>
            <p className="muted">{selected?.path.session_id}</p>
            {paths.length > 1 && (
              <label className="branch-picker">
                对话分支
                <select
                  className="select-input"
                  aria-label="对话分支"
                  value={selected?.path.id}
                  onChange={(e) => setParams({ path: e.target.value })}
                >
                  {paths.map(({ path }) => (
                    <option key={path.id} value={path.id}>
                      {path.session_id}
                    </option>
                  ))}
                </select>
              </label>
            )}
          </header>
          {selected && (
            <div className="messages">
              {selected.messages.map((message) => (
                <article key={message.id} className={`message ${message.role}`}>
                  <div className="message-label">
                    {message.role === "assistant" && <Sparkles size={16} />}
                    {message.role === "user"
                      ? "你"
                      : sources[conversation!.source]}
                  </div>
                  <div className="message-body">
                    <Markdown
                      remarkPlugins={[remarkGfm]}
                      skipHtml
                      components={{
                        img: ({ alt }) => (
                          <span className="muted">
                            [图片：{alt || "未加载"}]
                          </span>
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
                    {!message.content && (
                      <span className="muted">（空消息）</span>
                    )}
                  </div>
                </article>
              ))}
            </div>
          )}
          <footer className="chat-footer">
            {selected && (
              <a
                className="continue-conversation"
                href={selected.path.original_link}
                target="_blank"
                rel="noreferrer"
              >
                继续对话
                <ExternalLink size={16} />
              </a>
            )}
          </footer>
        </section>
      )}
    </>
  );
}
