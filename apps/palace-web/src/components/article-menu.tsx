import { useState, type FormEvent } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  ApiError,
  api,
  apiData,
  forgetArticleList,
  type Article,
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
import { ErrorState } from "./error-state";
import { FieldHelp } from "./field-help";
import { MoreMenu, type MenuAction } from "./more-menu";

type Fields = { title: string; url: string; author: string; date: string };

// The article page's menu. The body is someone else's writing and stays as imported; what the
// import may have got wrong (title, address, author, date) can be corrected, and the article can
// go with its images and videos.
export function ArticleMenu({
  article,
  reading,
  onDeleted,
}: {
  article: Article;
  // How the page is being read (只看标注 or the full text), listed first.
  reading: MenuAction;
  onDeleted: () => void;
}) {
  const [action, setAction] = useState<"edit" | "delete" | null>(null);
  const [fields, setFields] = useState<Fields>(() => fieldsOf(article));
  const client = useQueryClient();
  const update = useMutation({
    mutationFn: async () => {
      if (!fields.title.trim()) throw new Error("请填写标题。");
      if (new TextEncoder().encode(fields.title).length > 1024)
        throw new Error("标题不能超过 1024 字节。");
      try {
        return await apiData(
          api.PUT("/api/articles/{id}", {
            params: { path: { id: article.id } },
            body: {
              ...fields,
              author: fields.author.trim() ? fields.author : null,
            },
          }),
        );
      } catch (error) {
        // The one conflict an edit can meet is an address another article already has.
        if (error instanceof ApiError && error.status === 409)
          throw new ApiError(409, "另一篇文章已经使用这个原文地址。");
        throw error;
      }
    },
    onSuccess: async (updated) => {
      client.setQueryData(["article", article.id], updated);
      forgetArticleList(client);
      await client.invalidateQueries({ queryKey: ["timeline"] });
      setAction(null);
    },
  });
  const deletion = useMutation({
    mutationFn: () =>
      apiData(
        api.DELETE("/api/articles/{id}", {
          params: { path: { id: article.id } },
        }),
      ),
    onSuccess: async () => {
      setAction(null);
      onDeleted();
      client.removeQueries({ queryKey: ["article", article.id] });
      forgetArticleList(client);
      await client.invalidateQueries({ queryKey: ["timeline"] });
    },
  });
  const field = (name: keyof Fields) => ({
    value: fields[name],
    onChange: (event: { target: { value: string } }) =>
      setFields({ ...fields, [name]: event.target.value }),
  });
  return (
    <>
      <MoreMenu
        label="管理文章"
        actions={[
          reading,
          {
            label: "编辑文章",
            onSelect: () => {
              update.reset();
              setFields(fieldsOf(article));
              setAction("edit");
            },
          },
          {
            label: "删除文章",
            onSelect: () => {
              deletion.reset();
              setAction("delete");
            },
          },
        ]}
      />
      <Dialog
        open={action === "edit"}
        onOpenChange={(open) => {
          if (!open && !update.isPending) setAction(null);
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>编辑文章</DialogTitle>
            <DialogDescription>
              正文保持导入时的样子，不能修改。
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
                <Label htmlFor="article-title">标题</Label>
                <Input id="article-title" required {...field("title")} />
              </div>
              <div>
                <Label htmlFor="article-url">原文地址</Label>
                <Input id="article-url" type="url" required {...field("url")} />
              </div>
              <div>
                <Label htmlFor="article-author">作者</Label>
                <Input id="article-author" {...field("author")} />
              </div>
              <div>
                <div className="field-label">
                  <Label htmlFor="article-date">发布日期</Label>
                  <FieldHelp label="发布日期">
                    写成 2026-10-05，或带时区的时间，例如
                    2026-10-05T08:30:00+08:00。
                  </FieldHelp>
                </div>
                <Input id="article-date" required {...field("date")} />
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
            <DialogTitle>删除文章？</DialogTitle>
            <DialogDescription>
              文章和其中的图片、视频都会被删除，无法撤销。
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
              {deletion.isPending ? "正在删除…" : "删除文章"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}

// The form starts from what the server holds; its `date` is already in the form an edit takes.
function fieldsOf(article: Article): Fields {
  return {
    title: article.title,
    url: article.url,
    author: article.author ?? "",
    date: article.date,
  };
}
