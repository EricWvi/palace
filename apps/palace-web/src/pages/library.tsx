import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import { ArrowUpRight, MessageSquare, Plus, Search } from "lucide-react";
import { libraryOptions, formatTime, sources } from "@/lib/api";
import { useLibrary } from "@/lib/store";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ImportDialog } from "@/components/import-dialog";
import { ErrorState } from "@/components/error-state";
import { ConversationActions } from "@/components/conversation-actions";
export function LibraryPage() {
  const query = useQuery(libraryOptions);
  const [open, setOpen] = useState(false);
  const { search, setSearch } = useLibrary();
  const entries = [...(query.data ?? [])]
    .sort((a, b) => b.occurred_at - a.occurred_at || b.id.localeCompare(a.id))
    .filter((item) =>
      `${item.title} ${item.session_ids.join(" ")} ${sources[item.source]}`
        .toLowerCase()
        .includes(search.toLowerCase()),
    );
  return (
    <>
      <header className="topbar">
        <span>
          工作空间 / <strong>会话收藏</strong>
        </span>
        <span className="quiet">Palace Library</span>
      </header>
      <section className="library-content">
        <div className="page-heading">
          <div>
            <p className="eyebrow">COLLECT · REVISIT · CONNECT</p>
            <h1>
              收藏每一次好对话<span className="accent">.</span>
            </h1>
            <p className="muted">把散落的思考收在一起，让灵感有迹可循。</p>
          </div>
          <Button onClick={() => setOpen(true)}>
            <Plus size={17} />
            导入会话
          </Button>
        </div>
        <div className="library-tools">
          <span>
            <strong>全部会话</strong>{" "}
            <span className="count">{query.data?.length ?? 0}</span>
          </span>
          <div className="search">
            <Search size={16} />
            <Input
              aria-label="搜索会话"
              placeholder="搜索标题、来源或 Session ID"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
          </div>
        </div>
        <div className="list-caption">
          <span>会话</span>
          <span>对话发生时间 ↓</span>
        </div>
        {query.isPending ? (
          <p role="status" className="empty">
            正在加载会话…
          </p>
        ) : query.isError ? (
          <ErrorState error={query.error} retry={() => void query.refetch()} />
        ) : entries.length === 0 ? (
          <div className="empty">
            <MessageSquare size={36} />
            <h2>
              {search ? "没有找到匹配的会话" : "从一段值得收藏的对话开始"}
            </h2>
            <p>
              {search
                ? "试试其他标题或来源。"
                : "导入 ChatGPT、Gemini 或 Grok 的 JSON 会话。"}
            </p>
            {!search && (
              <Button variant="outline" onClick={() => setOpen(true)}>
                导入第一段会话
              </Button>
            )}
          </div>
        ) : (
          <div className="conversation-list">
            {entries.map((item) => (
              <div key={item.id} className="conversation-card">
                <Link
                  className="conversation-row"
                  to={`/conversations/${item.id}`}
                >
                  <span className={`source-icon ${item.source}`}>
                    {sources[item.source].slice(0, 1)}
                  </span>
                  <div className="conversation-info">
                    <h2>{item.title}</h2>
                    <p>
                      {sources[item.source]}
                      <span>·</span>
                      {item.message_count} 条消息<span>·</span>
                      <span>{item.path_count} 个分支</span>
                    </p>
                  </div>
                  <time dateTime={new Date(item.occurred_at).toISOString()}>
                    {formatTime(item.occurred_at)}
                  </time>
                  <ArrowUpRight size={18} />
                </Link>
                <ConversationActions conversation={item} />
              </div>
            ))}
          </div>
        )}
        <p className="library-footnote">每一段对话，都是下一次思考的起点。</p>
      </section>
      <ImportDialog open={open} onOpenChange={setOpen} />
    </>
  );
}
