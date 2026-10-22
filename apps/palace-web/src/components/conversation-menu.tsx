import { useState, type FormEvent } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  api,
  apiData,
  forgetConversationList,
  sources,
  type ConversationPath,
  type Message,
  type Source,
} from "@/lib/api";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "./ui/dialog";
import { BranchManager } from "./branch-manager";
import { ErrorState } from "./error-state";
import { MoreMenu, type MenuAction } from "./more-menu";
import { ImportDialog, type ImportResult } from "./import-dialog";

// The reading page is the only place a conversation is managed; the timeline only links here.
export function ConversationMenu({
  conversationId,
  source,
  path,
  messages,
  reading,
  onImported,
  onDeleted,
}: {
  conversationId: string;
  source: Source;
  // The path being read: the title edit applies to it, and new branches start from its title.
  path: ConversationPath;
  // The messages of that path, root first, which 导出分支 writes out.
  messages: Message[];
  // How the page is being read (只看标注 or the full text), listed first.
  reading: MenuAction;
  onImported: (result: ImportResult, occurredAt: number) => void;
  onDeleted: () => void;
}) {
  const [action, setAction] = useState<
    "update" | "branches" | "edit" | "delete" | null
  >(null);
  const [title, setTitle] = useState(path.title);
  const [chosen, setChosen] = useState<Source>(source);
  const client = useQueryClient();
  const refresh = () => {
    forgetConversationList(client);
    return Promise.all([
      client.invalidateQueries({ queryKey: ["conversation", conversationId] }),
      // Deleting a branch deletes the annotations on the messages only it had.
      client.invalidateQueries({
        queryKey: ["annotations", "conversation", conversationId],
      }),
      client.invalidateQueries({ queryKey: ["timeline"] }),
    ]);
  };
  const update = useMutation({
    mutationFn: () => {
      if (!title.trim()) throw new Error("请填写标题。");
      if (new TextEncoder().encode(title).length > 1024)
        throw new Error("标题不能超过 1024 字节。");
      return apiData(
        api.PUT("/api/conversations/{id}/paths/{path_id}/metadata", {
          params: { path: { id: conversationId, path_id: path.id } },
          body: { title, source: chosen },
        }),
      );
    },
    onSuccess: async () => {
      await refresh();
      setAction(null);
    },
  });
  const deletion = useMutation({
    mutationFn: () =>
      apiData(
        api.DELETE("/api/conversations/{id}", {
          params: { path: { id: conversationId } },
        }),
      ),
    onSuccess: async () => {
      setAction(null);
      onDeleted();
      client.removeQueries({ queryKey: ["conversation", conversationId] });
      forgetConversationList(client);
      await client.invalidateQueries({ queryKey: ["timeline"] });
    },
  });
  return (
    <>
      <MoreMenu
        label="管理对话"
        // Most frequent first, the one that cannot be undone last. Updating the branch being read
        // is what a reader comes back for, so it needs no detour through 管理分支.
        actions={[
          reading,
          { label: "更新分支", onSelect: () => setAction("update") },
          { label: "管理分支", onSelect: () => setAction("branches") },
          {
            label: "导出分支",
            // The file is the import format itself, so it can be uploaded again as a full history.
            onSelect: () => {
              const history = messages.map(({ role, content }) => ({
                role,
                content,
              }));
              const url = URL.createObjectURL(
                new Blob([JSON.stringify(history, null, 2)], {
                  type: "application/json",
                }),
              );
              const link = document.createElement("a");
              link.href = url;
              link.download = `${path.title}.json`;
              link.click();
              // Some browsers read the blob after click returns, so it is released on a later task.
              setTimeout(() => URL.revokeObjectURL(url));
            },
          },
          {
            label: "编辑对话",
            onSelect: () => {
              update.reset();
              setTitle(path.title);
              setChosen(source);
              setAction("edit");
            },
          },
          {
            label: "删除对话",
            onSelect: () => {
              deletion.reset();
              setAction("delete");
            },
          },
        ]}
      />
      {action === "update" && (
        <ImportDialog
          open
          mode={{ kind: "update", conversationId, source, path }}
          onOpenChange={(open) => {
            if (!open) setAction(null);
          }}
          onImported={(result, occurredAt) => {
            setAction(null);
            onImported(result, occurredAt);
          }}
        />
      )}
      {action === "branches" && (
        <BranchManager
          conversationId={conversationId}
          source={source}
          title={path.title}
          onImported={onImported}
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
            <DialogTitle>编辑对话</DialogTitle>
            <DialogDescription>
              标题只属于这一支；来源对整个对话的所有分支生效。
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
                <Label htmlFor="path-title">标题</Label>
                <Input
                  id="path-title"
                  required
                  value={title}
                  onChange={(event) => setTitle(event.target.value)}
                />
              </div>
              <div>
                <Label htmlFor="conversation-source">来源</Label>
                <select
                  id="conversation-source"
                  className="select-input"
                  value={chosen}
                  onChange={(event) => setChosen(event.target.value as Source)}
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
                  variant="ghost"
                  onClick={() => setAction(null)}
                >
                  取消
                </Button>
                <Button type="submit">
                  {update.isPending ? "正在保存…" : "保存"}
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
              这段对话的全部分支与消息都会被删除，无法撤销。
            </DialogDescription>
          </DialogHeader>
          {deletion.isError && <ErrorState error={deletion.error} />}
          <DialogFooter>
            <Button
              type="button"
              variant="ghost"
              onClick={() => setAction(null)}
            >
              取消
            </Button>
            <Button
              disabled={deletion.isPending}
              onClick={() => deletion.mutate()}
            >
              {deletion.isPending ? "正在删除…" : "删除对话"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
