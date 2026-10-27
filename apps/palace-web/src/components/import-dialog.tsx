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
import { FieldHelp } from "./field-help";
import { FileDrop } from "./file-drop";
import { MessageDrafts, messageDraft } from "./message-drafts";
import {
  api,
  apiData,
  forgetConversationList,
  serializeImport,
  sources,
  type Source,
  type ConversationPath,
} from "@/lib/api";
import { atCurrentTime } from "@/lib/day";
import { dollarMath } from "@/lib/math-delimiters";
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
// How an update supplies what it appends: a full history file, a file of new messages only, or
// new messages written in the dialog. Written messages always append after the head, so they need
// no switch of their own.
export type UpdateInput = "full" | "delta" | "messages";
// A new conversation needs no explanation beyond its fields; branch and update imports carry a
// rule about the uploaded history that the reader must know before choosing the file.
const headings = {
  conversation: ["导入对话", null],
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
  // How an update supplies its messages. It lives here because it changes the description, and it
  // returns to a full file on every opening: forgetting a delta on would append a later full
  // history twice.
  const [input, setInput] = useState<UpdateInput>("full");
  const [wasOpen, setWasOpen] = useState(open);
  if (open !== wasOpen) {
    setWasOpen(open);
    if (open) setInput("full");
  }
  const [title, fullDescription] = headings[mode.kind];
  const description =
    mode.kind !== "update" || input === "full"
      ? fullDescription
      : input === "delta"
        ? `只上传新增消息，会接在当前第 ${mode.path.message_count} 条消息之后。`
        : `追加的消息会接在当前第 ${mode.path.message_count} 条消息之后。`;
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className="import-dialog"
        // Radix warns about a dialog without a description unless told there is none on purpose.
        {...(description === null && { "aria-describedby": undefined })}
      >
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          {description !== null && (
            <DialogDescription>{description}</DialogDescription>
          )}
        </DialogHeader>
        {open && (
          <ImportForm
            mode={mode}
            input={input}
            onInputChange={setInput}
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
  input,
  onInputChange,
  onImported,
}: {
  mode: ImportMode;
  // Only an update offers a choice; every other mode reads a full file.
  input: UpdateInput;
  onInputChange: (input: UpdateInput) => void;
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
  const [drafts, setDrafts] = useState(() => [messageDraft("user")]);
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
      let history: string;
      if (input === "messages") {
        const blank = drafts.findIndex((draft) => !draft.content.trim());
        if (blank !== -1) throw new Error(`第 ${blank + 1} 条消息还没有内容。`);
        // The same JSON a file would carry, so written and uploaded deltas share one server path.
        // Replies pasted from ChatGPT carry escaped formula delimiters the reader cannot typeset;
        // the user's own words are kept as written, as the Automa export does.
        history = JSON.stringify(
          drafts.map(({ role, content }) => ({
            role,
            content: role === "assistant" ? dollarMath(content) : content,
          })),
        );
      } else {
        if (!file) throw new Error("请选择 JSON 文件。");
        await validateFile(file);
        history = await file.text();
      }
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
        input,
      ]);
      const key = attempt?.fingerprint === fingerprint ? attempt.key : uuidv7();
      setAttempt({ fingerprint, key });
      const fields = {
        title,
        history,
        occurred_at: date.getTime(),
        idempotency_key: key,
      };
      if (mode.kind === "branch") {
        return apiData(
          api.POST("/api/conversations/{id}/paths", {
            params: { path: { id: mode.conversationId } },
            body: { ...fields, session_id: session },
          }),
        );
      }
      if (mode.kind === "update") {
        return apiData(
          api.PUT("/api/conversations/{id}/paths/{path_id}", {
            params: {
              path: { id: mode.conversationId, path_id: mode.path.id },
            },
            // A delta names the head it was read against, so the server can refuse it if the path moved.
            body:
              input === "full"
                ? { ...fields, history_mode: "full" }
                : {
                    ...fields,
                    history_mode: "delta",
                    base_head_message_id: mode.path.head_message_id,
                  },
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
            // Only an update can write messages, so a new conversation always has its file here.
            history: file!,
          },
          bodySerializer: serializeImport,
        }),
      );
    },
    onSuccess: async (result) => {
      forgetConversationList(client);
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
          <div className="field-label">
            <Label htmlFor="session">Session ID</Label>
            <FieldHelp label="Session ID">
              {mode.kind === "update"
                ? "Session ID 保持不变。"
                : "同一来源的 Session ID 不可重复。"}
            </FieldHelp>
          </div>
          <Input
            id="session"
            disabled={mode.kind === "update"}
            required
            value={session}
            onChange={(e) => setSession(e.target.value)}
            placeholder="对话网址最后一段的 ID"
          />
        </div>
        <div>
          <Label>发生时间</Label>
          <DateTimePicker value={date} onChange={setDate} />
        </div>
        <div>
          <div className="field-label">
            {mode.kind === "update" ? (
              // The segment names the field, so the file input keeps its label for assistive tech only.
              <>
                <div className="segmented" role="group" aria-label="更新方式">
                  {(
                    [
                      ["file", "对话文件"],
                      ["messages", "追加消息"],
                    ] as const
                  ).map(([kind, label]) => (
                    <button
                      key={kind}
                      type="button"
                      aria-pressed={
                        (input === "messages") === (kind === "messages")
                      }
                      onClick={() => {
                        // Back on the file side the switch starts off again, the safer default.
                        onInputChange(
                          kind === "messages" ? "messages" : "full",
                        );
                        mutation.reset();
                      }}
                    >
                      {label}
                    </button>
                  ))}
                </div>
                {input !== "messages" && (
                  <Label htmlFor="history" className="sr-only">
                    对话文件
                  </Label>
                )}
              </>
            ) : (
              <Label htmlFor="history">对话文件</Label>
            )}
            {input !== "messages" && (
              <FieldHelp label="对话文件">
                <p>仅支持 JSON · 最大 8 MiB</p>
                <p>格式示例：</p>
                <pre>
                  {
                    '[{"role":"user","content":"你好"},\n {"role":"assistant","content":"你好！"}]'
                  }
                </pre>
              </FieldHelp>
            )}
            {mode.kind === "update" && input !== "messages" && (
              <span className="delta-toggle">
                <Label htmlFor="delta">增量导入</Label>
                <button
                  type="button"
                  role="switch"
                  id="delta"
                  className="switch"
                  aria-checked={input === "delta"}
                  onClick={() => {
                    onInputChange(input === "delta" ? "full" : "delta");
                    mutation.reset();
                  }}
                >
                  <span className="switch-thumb" />
                </button>
              </span>
            )}
          </div>
          {input === "messages" ? (
            <MessageDrafts
              drafts={drafts}
              assistant={sources[source]}
              onChange={(next) => {
                setDrafts(next);
                mutation.reset();
              }}
            />
          ) : (
            <FileDrop
              id="history"
              accept=".json,application/json"
              file={file}
              onFile={(next) => {
                setFile(next);
                mutation.reset();
              }}
            />
          )}
        </div>
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
