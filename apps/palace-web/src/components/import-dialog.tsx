import { useState, type FormEvent } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "react-router-dom";
import { FileJson, Upload } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { DateTimePicker } from "./date-time-picker";
import { ErrorState } from "./error-state";
import {
  request,
  sources,
  type Source,
  type ImportResult,
  type Conversation,
  type ConversationPath,
} from "@/lib/api";
import { validateFile } from "@/lib/import-file";
export type ImportMode =
  | { kind: "conversation" }
  | { kind: "branch"; conversation: Conversation }
  | { kind: "update"; conversation: Conversation; path: ConversationPath };
const newConversation: ImportMode = { kind: "conversation" };
export function ImportDialog({
  open,
  onOpenChange,
  mode = newConversation,
  onImported,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  mode?: ImportMode;
  onImported?: () => void;
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="import-dialog">
        <DialogHeader>
          <span className="dialog-icon">
            <Upload size={22} />
          </span>
          <DialogTitle>
            {mode.kind === "conversation"
              ? "收藏一段对话"
              : mode.kind === "branch"
                ? "新建分支"
                : "更新分支"}
          </DialogTitle>
          <DialogDescription>
            {mode.kind === "conversation"
              ? "从 JSON 文件导入，给这次思考一个名字。"
              : mode.kind === "branch"
                ? "上传完整 JSON，必须与已有路径共享包含 assistant 回复的前缀。"
                : "上传完整 JSON，只允许追加消息，不可修改或截短历史。"}
          </DialogDescription>
        </DialogHeader>
        {open && (
          <ImportForm
            mode={mode}
            onImported={onImported}
            onComplete={() => onOpenChange(false)}
          />
        )}
      </DialogContent>
    </Dialog>
  );
}
function ImportForm({
  onComplete,
  mode,
  onImported,
}: {
  onComplete: () => void;
  mode: ImportMode;
  onImported?: () => void;
}) {
  const [source, setSource] = useState<Source>(
    mode.kind === "conversation" ? "chatgpt" : mode.conversation.source,
  );
  const [title, setTitle] = useState(
    mode.kind === "conversation" ? "" : mode.conversation.title,
  );
  const [session, setSession] = useState(
    mode.kind === "update" ? mode.path.session_id : "",
  );
  const [date, setDate] = useState(() =>
    mode.kind === "update" ? new Date(mode.path.occurred_at) : new Date(),
  );
  const [file, setFile] = useState<File | null>(null);
  const [attempt, setAttempt] = useState<{
    fingerprint: string;
    key: string;
  } | null>(null);
  const client = useQueryClient();
  const navigate = useNavigate();
  const mutation = useMutation({
    mutationFn: async () => {
      if (!title.trim()) throw new Error("请填写会话标题。");
      if (
        !session ||
        /[\s/\\?#%:]/.test(session) ||
        session === "." ||
        session === ".."
      )
        throw new Error("请输入 Session ID 本身，而不是网址。");
      if (!file) throw new Error("请选择 JSON 文件。");
      await validateFile(file);
      const history = await file.text();
      // Reuse the key after an ambiguous network failure, but never for edited input.
      const fingerprint = JSON.stringify([
        source,
        title,
        session,
        date.getTime(),
        history,
        mode.kind,
        mode.kind === "conversation" ? null : mode.conversation.id,
        mode.kind === "update" ? mode.path.id : null,
      ]);
      const key =
        attempt?.fingerprint === fingerprint
          ? attempt.key
          : crypto.randomUUID();
      setAttempt({ fingerprint, key });
      if (mode.kind !== "conversation") {
        const base = `/api/conversations/${encodeURIComponent(mode.conversation.id)}/paths`;
        return request<ImportResult>(
          mode.kind === "branch"
            ? base
            : `${base}/${encodeURIComponent(mode.path.id)}`,
          {
            method: mode.kind === "branch" ? "POST" : "PUT",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
              ...(mode.kind === "branch" ? { session_id: session } : {}),
              history,
              occurred_at: date.getTime(),
              idempotency_key: key,
            }),
          },
        );
      }
      const body = new FormData();
      body.set("source", source);
      body.set("title", title);
      body.set("session_id", session);
      body.set("occurred_at", String(date.getTime()));
      body.set("idempotency_key", key);
      body.set("history", file);
      return request<ImportResult>("/api/import/file", {
        method: "POST",
        body,
      });
    },
    onSuccess: async (result) => {
      await Promise.all([
        client.invalidateQueries({ queryKey: ["conversations"] }),
        client.invalidateQueries({
          queryKey: ["conversation", result.conversation_id],
        }),
      ]);
      onComplete();
      if (onImported) {
        onImported();
        return;
      }
      navigate(
        `/conversations/${result.conversation_id}?path=${result.path_id}`,
      );
    },
  });
  function submit(event: FormEvent) {
    event.preventDefault();
    mutation.mutate();
  }
  return (
    <form onSubmit={submit}>
      <fieldset disabled={mutation.isPending} className="import-fields">
        <div>
          <Label htmlFor="title">自定义标题</Label>
          <Input
            id="title"
            disabled={mode.kind !== "conversation"}
            required
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="这段对话，关于什么？"
          />
        </div>
        <div>
          <Label htmlFor="source">会话来源</Label>
          <select
            id="source"
            disabled={mode.kind !== "conversation"}
            className="select-input"
            value={source}
            onChange={(e) => setSource(e.target.value as Source)}
          >
            {Object.entries(sources).map(([value, label]) => (
              <option key={value} value={value}>
                {label}
              </option>
            ))}
          </select>
        </div>
        <div>
          <Label htmlFor="session">来源网站 Session ID</Label>
          <Input
            id="session"
            disabled={mode.kind === "update"}
            required
            value={session}
            onChange={(e) => setSession(e.target.value)}
            placeholder="例如：会话网址最后一段的 ID"
          />
          <p className="field-hint">
            {mode.kind === "update"
              ? "Session ID 保持不变；发生时间可以修改。"
              : "同一来源的 Session ID 不可重复。"}
          </p>
        </div>
        <div>
          <Label>对话发生日期与时间</Label>
          <DateTimePicker value={date} onChange={setDate} />
          <p className="field-hint">
            使用本地时区 · {Intl.DateTimeFormat().resolvedOptions().timeZone}
          </p>
        </div>
        <div className="file-area">
          <FileJson size={27} />
          <Label htmlFor="history">{file?.name ?? "选择会话 JSON 文件"}</Label>
          <Input
            id="history"
            type="file"
            accept=".json,application/json"
            onChange={(e) => {
              setFile(e.target.files?.[0] ?? null);
              mutation.reset();
            }}
          />
          <p className="field-hint">仅支持 JSON · 最大 8 MiB</p>
        </div>
        <details className="format-help">
          <summary>查看 JSON 格式示例</summary>
          <pre>
            {
              '[{"role":"user","content":"你好"},\n {"role":"assistant","content":"你好！"}]'
            }
          </pre>
        </details>
        {mutation.isError && <ErrorState error={mutation.error} />}
        <Button type="submit" className="submit-import">
          <Upload size={16} />
          {mutation.isPending
            ? "正在导入…"
            : mode.kind === "conversation"
              ? "导入并查看会话"
              : mode.kind === "branch"
                ? "导入分支"
                : "保存更新"}
        </Button>
      </fieldset>
    </form>
  );
}
