import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Plus } from "lucide-react";
import { Tooltip } from "radix-ui";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "./ui/dialog";
import { Button } from "./ui/button";
import {
  ImportDialog,
  type ImportMode,
  type ImportResult,
} from "./import-dialog";
import { ErrorState } from "./error-state";
import {
  api,
  apiData,
  conversationOptions,
  forgetConversationList,
  type ConversationPath,
  type Source,
} from "@/lib/api";
import { resolvePaths } from "@/lib/conversation-tree";

// Every path of the conversation as one row: its title, then the two things done to a path.
// Rows follow the reading page's default order, newest update first, so the branch the page
// opens on by default heads the list.
export function BranchManager({
  conversationId,
  source,
  title,
  onImported,
  onClose,
}: {
  conversationId: string;
  source: Source;
  // The title of the path being read; a new branch starts with it.
  title: string;
  onImported: (result: ImportResult, occurredAt: number) => void;
  onClose: () => void;
}) {
  const [form, setForm] = useState<ImportMode | null>(null);
  const [deleting, setDeleting] = useState<ConversationPath | null>(null);
  const client = useQueryClient();
  const detail = useQuery(conversationOptions(conversationId));
  const deletion = useMutation({
    mutationFn: (path: ConversationPath) =>
      apiData(
        api.DELETE("/api/conversations/{id}/paths/{path_id}", {
          params: { path: { id: conversationId, path_id: path.id } },
        }),
      ),
    onSuccess: async () => {
      // The page falls back to a remaining path once the deleted one is gone from the tree.
      forgetConversationList(client);
      await Promise.all([
        client.invalidateQueries({
          queryKey: ["conversation", conversationId],
        }),
        client.invalidateQueries({ queryKey: ["timeline"] }),
      ]);
      setDeleting(null);
    },
  });
  const paths = useMemo(
    () =>
      detail.data ? resolvePaths(detail.data).map(({ path }) => path) : [],
    [detail.data],
  );
  // The last path goes with its conversation, through 删除对话, never on its own.
  const canDelete = paths.length > 1;
  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <DialogContent className="branch-dialog" aria-describedby={undefined}>
        <DialogHeader>
          <DialogTitle>管理分支</DialogTitle>
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
            {/* Adding a branch belongs to the list as a whole, so it heads the list beside the
                count rather than trailing after the last row. */}
            <div className="branch-list-head">
              <span>{paths.length} 个分支</span>
              <Button
                variant="ghost"
                size="sm"
                onClick={() =>
                  setForm({ kind: "branch", conversationId, source, title })
                }
              >
                <Plus />
                新建分支
              </Button>
            </div>
            <ul className="branch-list" aria-label="分支">
              {paths.map((path) => (
                <li key={path.id}>
                  <div className="branch-text">
                    <span className="branch-title" title={path.title}>
                      {path.title}
                    </span>
                    {/* Branches start with the title they were made from, so the session and
                        length are what tell two of the same name apart. */}
                    <span className="branch-meta" title={path.session_id}>
                      {path.session_id} · {path.message_count} 条消息
                    </span>
                  </div>
                  <Button
                    variant="ghost"
                    size="sm"
                    aria-label={`更新分支 ${path.session_id}`}
                    onClick={() =>
                      setForm({ kind: "update", conversationId, source, path })
                    }
                  >
                    更新
                  </Button>
                  {canDelete ? (
                    <Button
                      variant="ghost"
                      size="sm"
                      aria-label={`删除分支 ${path.session_id}`}
                      onClick={() => {
                        deletion.reset();
                        setDeleting(path);
                      }}
                    >
                      删除
                    </Button>
                  ) : (
                    // A disabled button takes no pointer or focus, so the reason it is off sits
                    // on a wrapper that does, and only shows when asked for.
                    <Tooltip.Provider delayDuration={150}>
                      <Tooltip.Root>
                        <Tooltip.Trigger asChild>
                          <span className="disabled-hint" tabIndex={0}>
                            <Button
                              variant="ghost"
                              size="sm"
                              aria-label={`删除分支 ${path.session_id}`}
                              disabled
                            >
                              删除
                            </Button>
                          </span>
                        </Tooltip.Trigger>
                        <Tooltip.Portal>
                          <Tooltip.Content
                            className="help-tip"
                            side="top"
                            sideOffset={6}
                          >
                            最后一个分支请通过“删除对话”移除。
                          </Tooltip.Content>
                        </Tooltip.Portal>
                      </Tooltip.Root>
                    </Tooltip.Provider>
                  )}
                </li>
              ))}
            </ul>
          </>
        )}
        {form && (
          <ImportDialog
            open
            mode={form}
            onOpenChange={(open) => {
              if (!open) setForm(null);
            }}
            onImported={(result, occurredAt) => {
              setForm(null);
              onClose();
              onImported(result, occurredAt);
            }}
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
                {deleting &&
                  `将删除“${deleting.title}”（${deleting.session_id}），其他分支共享的消息会保留。`}
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
