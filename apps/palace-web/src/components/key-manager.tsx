import { useEffect, useState, type FormEvent } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "./ui/dialog";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import { ErrorState } from "./error-state";
import { FieldHelp } from "./field-help";
import { accessTokensOptions, api, apiData, type AccessToken } from "@/lib/api";
import { dayParam } from "@/lib/day";

const copyLabels = {
  idle: "复制",
  copied: "已复制",
  failed: "复制失败",
} as const;

// Access tokens for the CLI and other services: give one a note to create it, delete one to
// revoke it. A token is only ever shown masked; the full value of the one just created lives in
// this component's state alone, so closing the dialog is what makes it unrecoverable.
export function KeyManager({ onClose }: { onClose: () => void }) {
  const [note, setNote] = useState("");
  const [fresh, setFresh] = useState<{ id: string; token: string } | null>(
    null,
  );
  const [deleting, setDeleting] = useState<AccessToken | null>(null);
  const client = useQueryClient();
  const tokens = useQuery(accessTokensOptions());
  const creation = useMutation({
    mutationFn: () =>
      apiData(api.POST("/api/access-tokens", { body: { note } })),
    onSuccess: async (created) => {
      setFresh({ id: created.access_token.id, token: created.token });
      setNote("");
      await client.invalidateQueries({ queryKey: ["access-tokens"] });
    },
  });
  const deletion = useMutation({
    mutationFn: (token: AccessToken) =>
      apiData(
        api.DELETE("/api/access-tokens/{id}", {
          params: { path: { id: token.id } },
        }),
      ),
    onSuccess: async (_, token) => {
      if (fresh?.id === token.id) setFresh(null);
      await client.invalidateQueries({ queryKey: ["access-tokens"] });
      setDeleting(null);
    },
  });
  function submit(event: FormEvent) {
    event.preventDefault();
    creation.mutate();
  }
  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <DialogContent className="branch-dialog" aria-describedby={undefined}>
        <DialogHeader>
          <DialogTitle>密钥管理</DialogTitle>
        </DialogHeader>
        <form onSubmit={submit}>
          <fieldset disabled={creation.isPending} className="key-form">
            <div className="field-label">
              <Label htmlFor="key-note">备注</Label>
              <FieldHelp label="密钥">
                <p>供 CLI 和第三方服务调用，放在请求头中：</p>
                <pre>Authorization: Bearer palace_pat_…</pre>
                <p>删除后立即失效。</p>
              </FieldHelp>
            </div>
            <div className="key-form-row">
              <Input
                id="key-note"
                required
                maxLength={100}
                value={note}
                onChange={(e) => {
                  setNote(e.target.value);
                  creation.reset();
                }}
                placeholder="用在哪里？"
              />
              <Button type="submit">
                {creation.isPending ? "正在生成…" : "生成"}
              </Button>
            </div>
            {creation.isError && <ErrorState error={creation.error} />}
          </fieldset>
        </form>
        {tokens.isPending ? (
          <p role="status">正在加载密钥…</p>
        ) : tokens.isError ? (
          <ErrorState
            error={tokens.error}
            retry={() => void tokens.refetch()}
          />
        ) : tokens.data.length === 0 ? (
          <p className="key-empty">还没有密钥。</p>
        ) : (
          <ul className="branch-list" aria-label="密钥">
            {tokens.data.map((token) => (
              // The actions share the note's line so the masked key gets the full width: its
              // last characters are what match it to a configuration, and must never be cut.
              <li key={token.id} className="key-item">
                <span className="branch-title" title={token.note}>
                  {token.note}
                </span>
                <span className="key-actions">
                  {fresh?.id === token.id && <CopyButton text={fresh.token} />}
                  <Button
                    variant="ghost"
                    size="sm"
                    aria-label={`删除密钥 ${token.note}`}
                    onClick={() => {
                      deletion.reset();
                      setDeleting(token);
                    }}
                  >
                    删除
                  </Button>
                </span>
                <span className="key-masked">{token.masked}</span>
                <span className="branch-meta">
                  {fresh?.id === token.id
                    ? "仅此一次可复制，关闭后无法再次查看"
                    : `创建于 ${dayParam(new Date(token.created_at))} · ${
                        token.last_used_at === null
                          ? "从未使用"
                          : `最近使用 ${dayParam(new Date(token.last_used_at))}`
                      }`}
                </span>
              </li>
            ))}
          </ul>
        )}
        <Dialog
          open={!!deleting}
          onOpenChange={(open) => {
            if (!open && !deletion.isPending) setDeleting(null);
          }}
        >
          <DialogContent>
            <DialogHeader>
              <DialogTitle>删除密钥？</DialogTitle>
              <DialogDescription>
                {deleting &&
                  `使用“${deleting.note}”的程序将立即无法访问，此操作不可撤销。`}
              </DialogDescription>
            </DialogHeader>
            {deletion.isError && <ErrorState error={deletion.error} />}
            <Button
              disabled={deletion.isPending}
              onClick={() => {
                if (deleting) deletion.mutate(deleting);
              }}
            >
              {deletion.isPending ? "正在删除…" : "确认删除密钥"}
            </Button>
          </DialogContent>
        </Dialog>
      </DialogContent>
    </Dialog>
  );
}

// Copies the full token without ever putting it on screen; the label says how it went, then
// returns to 复制 after a moment, as code blocks do.
function CopyButton({ text }: { text: string }) {
  const [state, setState] = useState<keyof typeof copyLabels>("idle");
  useEffect(() => {
    if (state === "idle") return;
    const timer = setTimeout(() => setState("idle"), 1500);
    return () => clearTimeout(timer);
  }, [state]);
  async function copy() {
    try {
      await navigator.clipboard.writeText(text);
      setState("copied");
    } catch {
      setState("failed");
    }
  }
  return (
    <Button variant="ghost" size="sm" aria-label="复制密钥" onClick={copy}>
      {copyLabels[state]}
    </Button>
  );
}
