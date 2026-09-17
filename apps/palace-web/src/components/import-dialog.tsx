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
import { request, sources, type Source, type ImportResult } from "@/lib/api";
import { validateFile } from "@/lib/import-file";
export function ImportDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="import-dialog">
        <DialogHeader>
          <span className="dialog-icon">
            <Upload size={22} />
          </span>
          <DialogTitle>收藏一段对话</DialogTitle>
          <DialogDescription>
            从 JSON 文件导入，给这次思考一个名字。
          </DialogDescription>
        </DialogHeader>
        {open && <ImportForm onComplete={() => onOpenChange(false)} />}
      </DialogContent>
    </Dialog>
  );
}
function ImportForm({ onComplete }: { onComplete: () => void }) {
  const [source, setSource] = useState<Source>("chatgpt");
  const [title, setTitle] = useState("");
  const [session, setSession] = useState("");
  const [date, setDate] = useState(() => new Date());
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
      // Reuse the key after an ambiguous network failure, but never for edited input.
      const fingerprint = JSON.stringify([
        source,
        title,
        session,
        date.getTime(),
        await file.text(),
      ]);
      const key =
        attempt?.fingerprint === fingerprint
          ? attempt.key
          : crypto.randomUUID();
      setAttempt({ fingerprint, key });
      const body = new FormData();
      body.set("source", source);
      body.set("title", title);
      body.set("session_id", session);
      body.set("imported_at", String(date.getTime()));
      body.set("idempotency_key", key);
      body.set("history", file);
      return request<ImportResult>("/api/import/file", {
        method: "POST",
        body,
      });
    },
    onSuccess: (result) => {
      void client.invalidateQueries({ queryKey: ["conversations"] });
      void client.invalidateQueries({
        queryKey: ["conversation", result.conversation_id],
      });
      onComplete();
      navigate(
        `/conversations/${result.conversation_id}?head=${result.head_message_id}`,
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
          <Label htmlFor="source">会话来源</Label>
          <select
            id="source"
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
            required
            value={session}
            onChange={(e) => setSession(e.target.value)}
            placeholder="例如：会话网址最后一段的 ID"
          />
          <p className="field-hint">相同来源与 ID 将合并消息，保留已有标题。</p>
        </div>
        <div>
          <Label htmlFor="title">自定义标题</Label>
          <Input
            id="title"
            required
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="这段对话，关于什么？"
          />
        </div>
        <div>
          <Label>导入日期与时间</Label>
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
          {mutation.isPending ? "正在导入…" : "导入并查看会话"}
        </Button>
      </fieldset>
    </form>
  );
}
