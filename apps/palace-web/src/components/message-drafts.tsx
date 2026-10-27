import { XIcon, PlusIcon } from "lucide-react";
import { v7 as uuidv7 } from "uuid";
import { Button } from "@/components/ui/button";

type Role = "user" | "assistant";

// One message written in the update dialog; the id only keys the row while it is being edited.
export type MessageDraft = { id: string; role: Role; content: string };

export function messageDraft(role: Role): MessageDraft {
  return { id: uuidv7(), role, content: "" };
}

// Messages to append, each a role and pasted Markdown. Roles are free, as in an uploaded file:
// a new row only starts on the other role from the row before it, because that is the usual turn.
export function MessageDrafts({
  drafts,
  assistant,
  onChange,
}: {
  drafts: MessageDraft[];
  // The name the reading page gives the assistant, so the roles read the same here.
  assistant: string;
  onChange: (drafts: MessageDraft[]) => void;
}) {
  function edit(id: string, change: Partial<MessageDraft>) {
    onChange(drafts.map((d) => (d.id === id ? { ...d, ...change } : d)));
  }
  return (
    <div className="message-drafts">
      {drafts.map((draft, index) => (
        <div key={draft.id} className="message-draft">
          <div className="message-draft-head">
            <select
              aria-label={`第 ${index + 1} 条消息的角色`}
              className="select-input"
              value={draft.role}
              onChange={(e) => edit(draft.id, { role: e.target.value as Role })}
            >
              <option value="user">你</option>
              <option value="assistant">{assistant}</option>
            </select>
            {drafts.length > 1 && (
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                aria-label={`删除第 ${index + 1} 条消息`}
                onClick={() =>
                  onChange(drafts.filter((d) => d.id !== draft.id))
                }
              >
                <XIcon />
              </Button>
            )}
          </div>
          <textarea
            aria-label={`第 ${index + 1} 条消息`}
            className="message-draft-input"
            rows={4}
            value={draft.content}
            onChange={(e) => edit(draft.id, { content: e.target.value })}
            placeholder="粘贴 Markdown"
          />
        </div>
      ))}
      <Button
        type="button"
        variant="ghost"
        size="sm"
        className="message-draft-add"
        onClick={() =>
          onChange([
            ...drafts,
            messageDraft(drafts.at(-1)?.role === "user" ? "assistant" : "user"),
          ])
        }
      >
        <PlusIcon />
        添加消息
      </Button>
    </div>
  );
}
