import { lazy, Suspense, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Plus } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "./ui/dialog";
import { Button } from "./ui/button";
import { ImportDialog, type ImportMode } from "./import-dialog";
import { ErrorState } from "./error-state";
import {
  request,
  type Conversation,
  type ConversationPath,
  type Detail,
} from "@/lib/api";
import { resolvePaths, userTree } from "@/lib/conversation-tree";

const BranchGraph = lazy(() =>
  import("./branch-graph").then((module) => ({ default: module.BranchGraph })),
);

export function BranchManager({
  conversation,
  onClose,
}: {
  conversation: Conversation;
  onClose: () => void;
}) {
  const [form, setForm] = useState<ImportMode | null>(null);
  const [deleting, setDeleting] = useState<ConversationPath | null>(null);
  const client = useQueryClient();
  const detail = useQuery({
    queryKey: ["conversation", conversation.id],
    queryFn: () =>
      request<Detail>(
        `/api/conversations/${encodeURIComponent(conversation.id)}`,
      ),
  });
  const deletion = useMutation({
    mutationFn: (path: ConversationPath) =>
      request(
        `/api/conversations/${encodeURIComponent(conversation.id)}/paths/${encodeURIComponent(path.id)}`,
        { method: "DELETE" },
      ),
    onSuccess: async () => {
      await Promise.all([
        client.invalidateQueries({
          queryKey: ["conversation", conversation.id],
        }),
        client.invalidateQueries({ queryKey: ["conversations"] }),
      ]);
      setDeleting(null);
    },
  });
  const tree = useMemo(
    () => (detail.data ? userTree(resolvePaths(detail.data)) : null),
    [detail.data],
  );
  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <DialogContent className="branch-dialog">
        <DialogHeader>
          <DialogTitle>分支管理</DialogTitle>
          <DialogDescription>
            {conversation.title} · 每个 Session 对应一条完整路径。
          </DialogDescription>
        </DialogHeader>
        {detail.isPending ? (
          <p role="status">正在加载分支…</p>
        ) : detail.isError ? (
          <ErrorState
            error={detail.error}
            retry={() => void detail.refetch()}
          />
        ) : (
          <>
            <Suspense
              fallback={
                <div className="branch-graph-loading" role="status">
                  正在绘制对话树…
                </div>
              }
            >
              <BranchGraph
                tree={tree!}
                canDelete={detail.data!.paths.length > 1}
                onUpdate={(path) =>
                  setForm({ kind: "update", conversation, path })
                }
                onDelete={(path) => {
                  deletion.reset();
                  setDeleting(path);
                }}
              />
            </Suspense>
            {detail.data!.paths.length === 1 && (
              <p className="field-hint">
                最后一个分支请通过卡片菜单的“删除对话”移除。
              </p>
            )}
            <div className="branch-toolbar">
              <p>拖动画布移动 · 滚轮缩放</p>
              <Button onClick={() => setForm({ kind: "branch", conversation })}>
                <Plus size={16} />
                新建分支
              </Button>
            </div>
          </>
        )}
        {form && (
          <ImportDialog
            open
            mode={form}
            onOpenChange={(open) => {
              if (!open) setForm(null);
            }}
            onImported={() => setForm(null)}
          />
        )}
        <Dialog
          open={!!deleting}
          onOpenChange={(open) => {
            if (!open && !deletion.isPending) setDeleting(null);
          }}
        >
          <DialogContent>
            <DialogHeader>
              <DialogTitle>删除分支？</DialogTitle>
              <DialogDescription>
                将删除 {deleting?.session_id}，其他分支共享的消息会保留。
              </DialogDescription>
            </DialogHeader>
            {deletion.isError && <ErrorState error={deletion.error} />}
            <Button
              disabled={deletion.isPending}
              onClick={() => {
                if (deleting) deletion.mutate(deleting);
              }}
            >
              {deletion.isPending ? "正在删除…" : "确认删除分支"}
            </Button>
          </DialogContent>
        </Dialog>
      </DialogContent>
    </Dialog>
  );
}
