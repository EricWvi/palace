import {
  infiniteQueryOptions,
  type InfiniteData,
  type QueryClient,
} from "@tanstack/react-query";
import createClient from "openapi-fetch";
import type { components, paths } from "./generated/api";

export type Source = components["schemas"]["Source"];
export type Moment = components["schemas"]["Moment"];
export type MomentKind = components["schemas"]["MomentKind"];
export type MomentOutline = components["schemas"]["MomentOutline"];
export type Message = components["schemas"]["Message"];
export type Detail = components["schemas"]["ConversationDetail"];
export type ConversationPath = components["schemas"]["ConversationPath"];
export type ConversationListItem =
  components["schemas"]["ConversationListItem"];
export type NoteListItem = components["schemas"]["NoteListItem"];
export type Article = components["schemas"]["Article"];
export type Annotation = components["schemas"]["Annotation"];
export type WebAnnotation = components["schemas"]["WebAnnotation"];
export type ArticleListItem = components["schemas"]["ArticleListItem"];
export type AccessToken = components["schemas"]["AccessToken"];
export type ListedJotting = components["schemas"]["ListedJotting"];
export type JottingOverview = components["schemas"]["JottingOverview"];
export type JottingRecall = components["schemas"]["JottingRecall"];
export type JottingPage = components["schemas"]["JottingPage"];
export type RecallState = components["schemas"]["RecallState"];
export type RecallStanding = components["schemas"]["RecallStanding"];
export type RecallNext = components["schemas"]["RecallNext"];
export type SleepNight = components["schemas"]["SleepNight"];
export type HabitBoard = components["schemas"]["HabitBoard"];
export const sources: Record<Source, string> = {
  chatgpt: "ChatGPT",
  gemini: "Gemini",
  grok: "Grok",
};

export const api = createClient<paths>({
  baseUrl: window.location.origin,
  credentials: "same-origin",
  // Resolve fetch at call time so tests and platform instrumentation can replace the transport.
  fetch: (request) => globalThis.fetch(request),
});

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
  }
}

// The caller's operation infers T; this unwraps its result without inventing a response type.
export async function apiData<T>(
  operation: Promise<{ data?: T; error?: unknown; response: Response }>,
): Promise<T> {
  const { data, error, response } = await operation;
  if (!response.ok) {
    const messages: Record<number, string> = {
      401: "请先登录后再继续。",
      403: "请求来源未被允许，请检查开发环境配置。",
      404: "找不到这段对话。",
      409: "请求冲突，请重新打开窗口后再试。",
      413: "文件或消息超过大小限制。",
    };
    let message = messages[response.status] ?? "服务暂时不可用，请稍后重试。";
    // Native extractor text and proxy errors do not have the JSON domain-error shape.
    if (typeof error === "object" && error !== null) {
      if ("error" in error && error.error === "session_already_exists") {
        message = "该来源已有相同的 Session ID，请检查来源或对应对话。";
      } else if ("error" in error && error.error === "path_head_moved") {
        message = "这条分支在别处有了新的更新，请刷新页面后重试。";
      } else if (
        "path" in error &&
        typeof error.path === "string" &&
        error.path &&
        "message" in error &&
        typeof error.message === "string"
      ) {
        message = `${error.path}：${error.message}`;
      }
    }
    throw new ApiError(response.status, message);
  }
  // 204 is the one success without a body; its operations type their data as never.
  if (data === undefined && response.status !== 204)
    throw new ApiError(response.status, "服务暂时不可用，请稍后重试。");
  return data as T;
}

// Keep the complete body typed before serialization; the browser owns Content-Type and boundary.
export function serializeImport(
  body: components["schemas"]["FileImport"],
): FormData {
  const form = new FormData();
  for (const [name, value] of Object.entries(body)) form.set(name, value);
  return form;
}

// One local day of moments; the key holds the day so each day caches separately.
export function timelineOptions(
  day: string,
  range: { start: number; end: number },
) {
  return {
    queryKey: ["timeline", day],
    queryFn: () =>
      apiData(api.GET("/api/timeline", { params: { query: range } })),
  };
}
// A day's moment ids and kinds, cheap enough to arrive before the cards and shape placeholders.
// It sits under the day's timeline key, so invalidating a day's timeline invalidates it too.
export function outlineOptions(
  day: string,
  range: { start: number; end: number },
) {
  return {
    queryKey: ["timeline", day, "outline"],
    queryFn: () =>
      apiData(api.GET("/api/timeline/outline", { params: { query: range } })),
  };
}
export function conversationOptions(id: string) {
  return {
    queryKey: ["conversation", id],
    queryFn: () =>
      apiData(api.GET("/api/conversations/{id}", { params: { path: { id } } })),
  };
}

// 摘星's conversation list, one cursor page at a time; each search keeps its own pages.
export function conversationListOptions(q: string) {
  return infiniteQueryOptions({
    queryKey: ["conversations", q],
    queryFn: ({ pageParam }) =>
      apiData(
        api.GET("/api/conversations", {
          params: { query: { q: q || undefined, cursor: pageParam } },
        }),
      ),
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (page) => page.next_cursor ?? undefined,
  });
}
// 摘星's note list. Notes change outside Palace, so nothing here invalidates it; a fresh visit
// refetches it like any other query.
export function noteListOptions(q: string) {
  return infiniteQueryOptions({
    queryKey: ["notes", q],
    queryFn: ({ pageParam }) =>
      apiData(
        api.GET("/api/notes", {
          params: { query: { q: q || undefined, cursor: pageParam } },
        }),
      ),
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (page) => page.next_cursor ?? undefined,
  });
}
// One note; its id holds `/` and travels as a single encoded path segment.
export function noteOptions(id: string) {
  return {
    queryKey: ["note", id],
    queryFn: () =>
      apiData(api.GET("/api/notes/{id}", { params: { path: { id } } })),
  };
}
// The note page's address keeps the id's `/` as path separators and encodes each segment.
export function notePath(id: string) {
  return `/notes/${id.split("/").map(encodeURIComponent).join("/")}`;
}
// 摘星's article list, latest publish date first; each search keeps its own pages.
export function articleListOptions(q: string) {
  return infiniteQueryOptions({
    queryKey: ["articles", q],
    queryFn: ({ pageParam }) =>
      apiData(
        api.GET("/api/articles", {
          params: { query: { q: q || undefined, cursor: pageParam } },
        }),
      ),
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (page) => page.next_cursor ?? undefined,
  });
}
export function articleOptions(id: string) {
  return {
    queryKey: ["article", id],
    queryFn: () =>
      apiData(api.GET("/api/articles/{id}", { params: { path: { id } } })),
  };
}
// What a reading page annotates: an article body, or the messages of a conversation.
export type AnnotationTarget = {
  kind: "article" | "conversation";
  id: string;
};
// Every annotation on one article or one conversation; changes write the list back in place.
export function annotationsOptions({ kind, id }: AnnotationTarget) {
  return {
    queryKey: ["annotations", kind, id],
    queryFn: () =>
      apiData(
        kind === "article"
          ? api.GET("/api/articles/{id}/annotations", {
              params: { path: { id } },
            })
          : api.GET("/api/conversations/{id}/annotations", {
              params: { path: { id } },
            }),
      ),
  };
}
// The web annotations still on the pages bound to an article: those the article page has yet
// to migrate, then those it could not place.
export function webAnnotationsOptions(id: string) {
  return {
    queryKey: ["web-annotations", id],
    queryFn: () =>
      apiData(
        api.GET("/api/articles/{id}/web-annotations", {
          params: { path: { id } },
        }),
      ),
  };
}
// Dropped rather than refetched in place after an edit or deletion, for the same reason as the
// conversation list: an old page shown meanwhile would offer a deleted article or an old title.
export function forgetArticleList(client: QueryClient) {
  client.removeQueries({ queryKey: ["articles"] });
}
// The owner's access tokens, masked; a full token never enters the query cache.
export function accessTokensOptions() {
  return {
    queryKey: ["access-tokens"],
    queryFn: () => apiData(api.GET("/api/access-tokens")),
  };
}
// After any import, rename or deletion the list is dropped rather than refetched in place: an
// old page shown while the new one loads would briefly offer a deleted path or an old title.
export function forgetConversationList(client: QueryClient) {
  client.removeQueries({ queryKey: ["conversations"] });
}

// 回响's wall, newest first, one cursor page at a time; each tag and search keeps its own pages.
export function jottingListOptions(tag: string, q: string) {
  return infiniteQueryOptions({
    queryKey: ["jottings", "list", tag, q],
    queryFn: ({ pageParam }) =>
      apiData(
        api.GET("/api/jottings", {
          params: {
            query: {
              tag: tag || undefined,
              q: q || undefined,
              cursor: pageParam,
            },
          },
        }),
      ),
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (page) => page.next_cursor ?? undefined,
  });
}
// Counts for 回响's left column, per day from `from` (`YYYY-MM-DD`) through today.
export function jottingOverviewOptions(from: string) {
  return {
    queryKey: ["jottings", "overview", from],
    queryFn: () =>
      apiData(
        api.GET("/api/jottings/overview", { params: { query: { from } } }),
      ),
  };
}
// Today's recall pile. The server keeps it as drawn all day, so it is not refetched while the
// page stays open; completing a card marks it done in this copy.
export function jottingRecallOptions() {
  return {
    queryKey: ["jottings", "recall"],
    queryFn: () => apiData(api.GET("/api/jottings/recall")),
    staleTime: Infinity,
  };
}
// Completes a card of the pile of `date`: 温故 keeps the jotting in recall, 束阁 retires it.
export function completeRecall(date: string, id: string, next: RecallNext) {
  return apiData(
    api.POST("/api/jottings/recall/{date}/{id}/done", {
      params: { path: { date, id } },
      body: { next },
    }),
  );
}
// Takes a jotting out of recall, or back in.
export function setJottingRecall(id: string, recall: RecallState) {
  return apiData(
    api.PUT("/api/jottings/{id}/recall", {
      params: { path: { id } },
      body: { recall },
    }),
  );
}
// Writes a jotting's recall state, and whether its card is done, into every cached copy, so
// the wall, the pile and an open card agree without refetching.
export function patchRecall(
  client: QueryClient,
  id: string,
  change: { recall: RecallStanding; done?: true },
) {
  client.setQueriesData<InfiniteData<JottingPage>>(
    { queryKey: ["jottings", "list"] },
    (data) =>
      data && {
        ...data,
        pages: data.pages.map((page) => ({
          ...page,
          jottings: page.jottings.map((jotting) =>
            jotting.id === id ? { ...jotting, recall: change.recall } : jotting,
          ),
        })),
      },
  );
  client.setQueryData<JottingRecall>(
    ["jottings", "recall"],
    (data) =>
      data && {
        ...data,
        jottings: data.jottings.map((card) =>
          card.id === id ? { ...card, ...change } : card,
        ),
      },
  );
}
// Sets a jotting's title, tags and body as of the version the page read; a newer version
// answers 409, and the page must not overwrite it.
export function editJotting(
  id: string,
  version: string,
  edit: { title: string | null; tags: string[]; body: string },
) {
  return apiData(
    api.PUT("/api/jottings/{id}", {
      params: { path: { id } },
      body: { version, ...edit },
    }),
  );
}
// Deletes a jotting as of the version the page read.
export function deleteJotting(id: string, version: string) {
  return apiData(
    api.DELETE("/api/jottings/{id}", {
      params: { path: { id }, query: { version } },
    }),
  );
}
// Writes an edited jotting into every cached copy, the wall's and the pile's; its tags may
// have changed the counts.
export function patchJotting(
  client: QueryClient,
  id: string,
  change: Pick<
    ListedJotting,
    "title" | "tags" | "body" | "version" | "pending"
  >,
) {
  client.setQueriesData<InfiniteData<JottingPage>>(
    { queryKey: ["jottings", "list"] },
    (data) =>
      data && {
        ...data,
        pages: data.pages.map((page) => ({
          ...page,
          jottings: page.jottings.map((jotting) =>
            jotting.id === id ? { ...jotting, ...change } : jotting,
          ),
        })),
      },
  );
  client.setQueryData<JottingRecall>(
    ["jottings", "recall"],
    (data) =>
      data && {
        ...data,
        jottings: data.jottings.map((card) =>
          card.id === id ? { ...card, ...change } : card,
        ),
      },
  );
  void client.invalidateQueries({ queryKey: ["jottings", "overview"] });
}
// Takes a deleted jotting out of every cached copy; the wall keeps the other cards where they
// are, and the counts follow.
export function dropJotting(client: QueryClient, id: string) {
  client.setQueriesData<InfiniteData<JottingPage>>(
    { queryKey: ["jottings", "list"] },
    (data) => {
      if (!data) return data;
      // Every page carries the same total, so each loses the one card wherever it was.
      const held = data.pages.some((page) =>
        page.jottings.some((jotting) => jotting.id === id),
      );
      return {
        ...data,
        pages: data.pages.map((page) => ({
          ...page,
          jottings: page.jottings.filter((jotting) => jotting.id !== id),
          total: held ? page.total - 1 : page.total,
        })),
      };
    },
  );
  client.setQueryData<JottingRecall>(
    ["jottings", "recall"],
    (data) =>
      data && {
        ...data,
        jottings: data.jottings.filter((card) => card.id !== id),
      },
  );
  void client.invalidateQueries({ queryKey: ["jottings", "overview"] });
}
// A new jotting changes the wall and the counts; today's recall pile never includes it.
export function refreshJottings(client: QueryClient) {
  void client.invalidateQueries({ queryKey: ["jottings", "list"] });
  void client.invalidateQueries({ queryKey: ["jottings", "overview"] });
}

// 行事's lists, open tasks and the tasks finished on `day`, the reader's today. Every change
// writes the cached board in place first, so the page answers at once; a failed change refetches
// it.
export function todoBoardOptions(day: string) {
  return {
    queryKey: ["todos", "board", day],
    queryFn: () =>
      apiData(api.GET("/api/todos", { params: { query: { day } } })),
  };
}
// One list's finished tasks, fetched when the list is open.
export function completedTodosOptions(list: string) {
  return {
    queryKey: ["todos", "completed", list],
    queryFn: () =>
      apiData(
        api.GET("/api/todo-lists/{id}/completed", {
          params: { path: { id: list } },
        }),
      ),
  };
}
// A task's body, read when its panel opens and never shared with another view.
export function todoBodyOptions(id: string) {
  return {
    queryKey: ["todos", "body", id],
    queryFn: () =>
      apiData(api.GET("/api/todos/{id}/body", { params: { path: { id } } })),
    staleTime: 0,
  };
}

// The running sleep. The page asks for it fresh, since another device may have started or ended
// one since.
export function sleepRunningOptions() {
  return {
    queryKey: ["sleep", "running"],
    queryFn: () => apiData(api.GET("/api/sleep/running")),
    staleTime: 0,
  };
}
// Nights waking in a run of local days; the key names the days so each run caches on its own.
export function sleepNightsOptions(
  from: string,
  to: string,
  range: { start: number; end: number },
) {
  return {
    queryKey: ["sleep", "nights", from, to],
    queryFn: () =>
      apiData(api.GET("/api/sleep/nights", { params: { query: range } })),
  };
}
// A night changed: every run of nights and every day's timeline may hold it.
export async function refreshSleep(client: QueryClient) {
  await Promise.all([
    client.invalidateQueries({ queryKey: ["sleep"] }),
    client.invalidateQueries({ queryKey: ["timeline"] }),
  ]);
}

// 日课 over a run of local days, `YYYY-MM-DD` both ends included. A tap writes the cached run in
// place first, so the seal answers at once; a failed write refetches it.
export function habitBoardOptions(from: string, to: string) {
  return {
    queryKey: ["habits", from, to],
    queryFn: () =>
      apiData(api.GET("/api/habits", { params: { query: { from, to } } })),
  };
}
// 工夫's types, in the order they were added.
export function activityTypesOptions() {
  return {
    queryKey: ["activities", "types"],
    queryFn: () => apiData(api.GET("/api/activity-types")),
  };
}
// 工夫 starting in a run of local days; the key names the days so each run caches on its own.
export function activitiesOptions(
  from: string,
  to: string,
  range: { start: number; end: number },
) {
  return {
    queryKey: ["activities", "list", from, to],
    queryFn: () =>
      apiData(api.GET("/api/activities", { params: { query: range } })),
  };
}
// An activity changed: every run of activities and every day's timeline may hold it.
export async function refreshActivities(client: QueryClient) {
  await Promise.all([
    client.invalidateQueries({ queryKey: ["activities"] }),
    client.invalidateQueries({ queryKey: ["timeline"] }),
  ]);
}
