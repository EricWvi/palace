import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { DropdownMenu } from "radix-ui";
import { MoreHorizontal } from "lucide-react";
import { request, type Conversation } from "@/lib/api";
import { Button } from "./ui/button";
import {
  Dialog,
  DialogContent,
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
  const [action, setAction] = useState<"branches" | "delete" | null>(null);
  const client = useQueryClient();
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
