import { useState, type FormEvent } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { DropdownMenu } from "radix-ui";
import { MoreHorizontal } from "lucide-react";
import { request, sources, type Conversation, type Source } from "@/lib/api";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "./ui/dialog";
import { BranchManager } from "./branch-manager";
import { ErrorState } from "./error-state";

export function ConversationActions({
  conversation,
}: {
  conversation: Conversation;
}) {
  const [action, setAction] = useState<"branches" | "edit" | "delete" | null>(
    null,
  );
  const [title, setTitle] = useState(conversation.title);
  const [source, setSource] = useState<Source>(conversation.source);
  const client = useQueryClient();
  const update = useMutation({
    mutationFn: () => {
      if (!title.trim()) throw new Error("请填写会话标题。");
      if (new TextEncoder().encode(title).length > 1024)
        throw new Error("会话标题不能超过 1024 字节。");
      return request<Conversation>(
        `/api/conversations/${encodeURIComponent(conversation.id)}`,
        {
          method: "PUT",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ title, source }),
        },
      );
    },
    onSuccess: async () => {
      await Promise.all([
        client.invalidateQueries({ queryKey: ["conversations"] }),
        client.invalidateQueries({
          queryKey: ["conversation", conversation.id],
        }),
      ]);
      setAction(null);
    },
  });
  const deletion = useMutation({
    mutationFn: () =>
      request(`/api/conversations/${encodeURIComponent(conversation.id)}`, {
        method: "DELETE",
      }),
    onSuccess: async () => {
      client.removeQueries({ queryKey: ["conversation", conversation.id] });
      await client.invalidateQueries({ queryKey: ["conversations"] });
      setAction(null);
    },
  });
  return (
    <>
      <DropdownMenu.Root>
        <DropdownMenu.Trigger asChild>
          <Button
            variant="ghost"
            size="icon"
            aria-label={`会话菜单 ${conversation.title}`}
          >
            <MoreHorizontal size={18} />
          </Button>
        </DropdownMenu.Trigger>
        <DropdownMenu.Portal>
          <DropdownMenu.Content
            className="conversation-menu"
            align="end"
            sideOffset={6}
            onCloseAutoFocus={(event) => {
              if (action) event.preventDefault();
            }}
          >
            <DropdownMenu.Item onSelect={() => setAction("branches")}>
              分支管理
            </DropdownMenu.Item>
            <DropdownMenu.Item
              onSelect={() => {
                update.reset();
                setTitle(conversation.title);
                setSource(conversation.source);
                setAction("edit");
              }}
            >
              编辑会话
            </DropdownMenu.Item>
            <DropdownMenu.Item
              onSelect={() => {
                deletion.reset();
                setAction("delete");
              }}
            >
              删除对话
            </DropdownMenu.Item>
          </DropdownMenu.Content>
        </DropdownMenu.Portal>
      </DropdownMenu.Root>
      {action === "branches" && (
        <BranchManager
          conversation={conversation}
          onClose={() => setAction(null)}
        />
      )}
      <Dialog
        open={action === "edit"}
        onOpenChange={(open) => {
          if (!open && !update.isPending) setAction(null);
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>编辑会话</DialogTitle>
            <DialogDescription>
              修正标题或导入时选错的消息来源。
            </DialogDescription>
          </DialogHeader>
          <form
            onSubmit={(event: FormEvent) => {
              event.preventDefault();
              update.mutate();
            }}
          >
            <fieldset disabled={update.isPending} className="import-fields">
              <div>
                <Label htmlFor={`conversation-title-${conversation.id}`}>
                  会话标题
                </Label>
                <Input
                  id={`conversation-title-${conversation.id}`}
                  required
                  value={title}
                  onChange={(event) => setTitle(event.target.value)}
                />
              </div>
              <div>
                <Label htmlFor={`conversation-source-${conversation.id}`}>
                  消息来源
                </Label>
                <select
                  id={`conversation-source-${conversation.id}`}
                  className="select-input"
                  value={source}
                  onChange={(event) => setSource(event.target.value as Source)}
                >
                  {Object.entries(sources).map(([value, label]) => (
                    <option key={value} value={value}>
                      {label}
                    </option>
                  ))}
                </select>
              </div>
              {update.isError && <ErrorState error={update.error} />}
              <DialogFooter>
                <Button
                  type="button"
                  variant="outline"
                  onClick={() => setAction(null)}
                >
                  取消
                </Button>
                <Button type="submit">
                  {update.isPending ? "正在保存…" : "保存更改"}
                </Button>
              </DialogFooter>
            </fieldset>
          </form>
        </DialogContent>
      </Dialog>
      <Dialog
        open={action === "delete"}
        onOpenChange={(open) => {
          if (!open && !deletion.isPending) setAction(null);
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>删除对话？</DialogTitle>
            <DialogDescription>
              “{conversation.title}”的全部分支与消息将被删除，无法撤销。
            </DialogDescription>
          </DialogHeader>
          {deletion.isError && <ErrorState error={deletion.error} />}
          <Button
            disabled={deletion.isPending}
            onClick={() => deletion.mutate()}
          >
            {deletion.isPending ? "正在删除…" : "确认删除对话"}
          </Button>
        </DialogContent>
      </Dialog>
    </>
  );
}
