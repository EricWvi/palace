import { useQuery } from "@tanstack/react-query";
import { Link, useParams, useSearchParams } from "react-router-dom";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { ArrowLeft, ExternalLink, Sparkles } from "lucide-react";
import { request, sources, type Detail, type Message } from "@/lib/api";
import { ErrorState } from "@/components/error-state";
export function ConversationPage() {
  const { id } = useParams();
  const [params, setParams] = useSearchParams();
  const detail = useQuery({
    queryKey: ["conversation", id],
    queryFn: () =>
      request<Detail>(`/api/conversations/${encodeURIComponent(id!)}`),
  });
  const messages = detail.data?.messages ?? [];
  const parents = new Set(messages.map((m) => m.parent_message_id));
  const leaves = messages
    .filter((m) => !parents.has(m.id))
    .sort((a, b) => b.created_order - a.created_order);
  const head = params.get("head") ?? leaves[0]?.id;
  const path = useQuery({
    queryKey: ["conversation", id, "path", head],
    queryFn: () =>
      request<Message[]>(
        `/api/conversations/${encodeURIComponent(id!)}/paths/${encodeURIComponent(head!)}`,
      ),
    enabled: !!detail.data && !!head,
  });
  const conversation = detail.data?.conversation;
  return (
    <>
      <header className="topbar">
        <Link to="/" className="back-link">
          <ArrowLeft size={16} />
          返回会话收藏
        </Link>
        {detail.data && (
          <a
            className="back-link"
            href={detail.data.original_link}
            target="_blank"
            rel="noreferrer"
          >
            查看原会话
            <ExternalLink size={14} />
          </a>
        )}
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
            <p className="muted">{conversation!.session_id}</p>
            {leaves.length > 1 && (
              <label className="branch-picker">
                对话分支
                <select
                  className="select-input"
                  aria-label="对话分支"
                  value={head}
                  onChange={(e) => setParams({ head: e.target.value })}
                >
                  {leaves.map((leaf, index) => (
                    <option key={leaf.id} value={leaf.id}>
                      分支 {index + 1} · {leaf.content.slice(0, 35) || "空消息"}
                    </option>
                  ))}
                </select>
              </label>
            )}
          </header>
          {path.isError ? (
            <ErrorState error={path.error} retry={() => void path.refetch()} />
          ) : path.isPending && head ? (
            <p role="status">正在加载消息…</p>
          ) : (
            <div className="messages">
              {path.data?.map((message) => (
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
            已归档的对话 · 在这里重温，在下一次思考中继续
          </footer>
        </section>
      )}
    </>
  );
}
