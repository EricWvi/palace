import { useState, type FormEvent } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { v7 as uuidv7 } from "uuid";
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
  api,
  apiData,
  serializeImport,
  sources,
  type Source,
  type ConversationPath,
} from "@/lib/api";
import { atCurrentTime } from "@/lib/day";
import { validateFile } from "@/lib/import-file";
import type { components } from "@/lib/generated/api";

export type ImportResult = components["schemas"]["ImportResult"];
export type ImportMode =
  // `day` is the timeline day the import was started from.
  | { kind: "conversation"; day: Date }
  // `title` is the path being read when the branch was started; the new branch starts with it.
  | { kind: "branch"; conversationId: string; source: Source; title: string }
  | {
      kind: "update";
      conversationId: string;
      source: Source;
      path: ConversationPath;
    };
const headings = {
  conversation: ["导入对话", "上传对话的 JSON 文件，并为它命名。"],
  branch: [
    "新建分支",
    "上传完整 JSON，必须与已有分支共享到某条回复为止的开头。",
  ],
  update: ["更新分支", "上传完整 JSON，只能在原有消息之后追加。"],
} as const;

export function ImportDialog({
  open,
  onOpenChange,
  mode,
  onImported,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  mode: ImportMode;
  // Receives the submitted occurrence time, which decides the day the reader returns to.
  onImported: (result: ImportResult, occurredAt: number) => void;
}) {
  const [title, description] = headings[mode.kind];
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="import-dialog">
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>{description}</DialogDescription>
        </DialogHeader>
        {open && (
          <ImportForm
            mode={mode}
            onImported={(result, occurredAt) => {
              onOpenChange(false);
              onImported(result, occurredAt);
            }}
          />
        )}
      </DialogContent>
    </Dialog>
  );
}
function ImportForm({
  mode,
  onImported,
}: {
  mode: ImportMode;
  onImported: (result: ImportResult, occurredAt: number) => void;
}) {
  const [source, setSource] = useState<Source>(
    mode.kind === "conversation" ? "chatgpt" : mode.source,
  );
  const [title, setTitle] = useState(
    mode.kind === "conversation"
      ? ""
      : mode.kind === "branch"
        ? mode.title
        : mode.path.title,
  );
  const [session, setSession] = useState(
    mode.kind === "update" ? mode.path.session_id : "",
  );
  const [date, setDate] = useState(() =>
    mode.kind === "update"
      ? new Date(mode.path.occurred_at)
      : mode.kind === "conversation"
        ? atCurrentTime(mode.day)
        : new Date(),
  );
  const [file, setFile] = useState<File | null>(null);
  const [attempt, setAttempt] = useState<{
    fingerprint: string;
    key: string;
  } | null>(null);
  const client = useQueryClient();
  const mutation = useMutation({
    mutationFn: async () => {
      if (!title.trim()) throw new Error("请填写标题。");
      if (new TextEncoder().encode(title).length > 1024)
        throw new Error("标题不能超过 1024 字节。");
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
        mode.kind === "conversation" ? null : mode.conversationId,
        mode.kind === "update" ? mode.path.id : null,
      ]);
      const key = attempt?.fingerprint === fingerprint ? attempt.key : uuidv7();
      setAttempt({ fingerprint, key });
      const input = {
        title,
        history,
        occurred_at: date.getTime(),
        idempotency_key: key,
      };
      if (mode.kind === "branch") {
        return apiData(
          api.POST("/api/conversations/{id}/paths", {
            params: { path: { id: mode.conversationId } },
            body: { ...input, session_id: session },
          }),
        );
      }
      if (mode.kind === "update") {
        return apiData(
          api.PUT("/api/conversations/{id}/paths/{path_id}", {
            params: {
              path: { id: mode.conversationId, path_id: mode.path.id },
            },
            body: input,
          }),
        );
      }
      return apiData(
        api.POST("/api/import/file", {
          body: {
            source,
            title,
            session_id: session,
            occurred_at: String(date.getTime()),
            idempotency_key: key,
            history: file,
          },
          bodySerializer: serializeImport,
        }),
      );
    },
    onSuccess: async (result) => {
      await Promise.all([
        client.invalidateQueries({ queryKey: ["timeline"] }),
        client.invalidateQueries({
          queryKey: ["conversation", result.conversation_id],
        }),
      ]);
      onImported(result, date.getTime());
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
          <Label htmlFor="title">标题</Label>
          <Input
            id="title"
            required
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="这段对话，关于什么？"
          />
        </div>
        <div>
          <Label htmlFor="source">来源</Label>
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
          <Label htmlFor="session">Session ID</Label>
          <Input
            id="session"
            disabled={mode.kind === "update"}
            required
            value={session}
            onChange={(e) => setSession(e.target.value)}
            placeholder="对话网址最后一段的 ID"
          />
          <p className="field-hint">
            {mode.kind === "update"
              ? "Session ID 保持不变。"
              : "同一来源的 Session ID 不可重复。"}
          </p>
        </div>
        <div>
          <Label>发生时间</Label>
          <DateTimePicker value={date} onChange={setDate} />
          <p className="field-hint">
            本地时区 · {Intl.DateTimeFormat().resolvedOptions().timeZone}
          </p>
        </div>
        <div>
          <Label htmlFor="history">{file?.name ?? "选择对话 JSON 文件"}</Label>
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
          <summary>JSON 格式示例</summary>
          <pre>
            {
              '[{"role":"user","content":"你好"},\n {"role":"assistant","content":"你好！"}]'
            }
          </pre>
        </details>
        {mutation.isError && <ErrorState error={mutation.error} />}
        <Button type="submit" className="submit-import">
          {mutation.isPending
            ? "正在导入…"
            : mode.kind === "conversation"
              ? "导入"
              : mode.kind === "branch"
                ? "导入分支"
                : "保存更新"}
        </Button>
      </fieldset>
    </form>
  );
}
