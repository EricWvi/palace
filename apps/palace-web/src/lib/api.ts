import createClient from "openapi-fetch";
import type { components, paths } from "./generated/api";

export type Source = components["schemas"]["Source"];
export type ConversationMetadata =
  components["schemas"]["ConversationMetadataResponse"];
export type Summary = components["schemas"]["ConversationSummary"];
export type Message = components["schemas"]["Message"];
export type Detail = components["schemas"]["ConversationDetail"];
export type ConversationPath = components["schemas"]["ConversationPath"];
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
      404: "找不到这段会话。",
      409: "导入请求冲突，请重新打开导入窗口。",
      413: "文件或消息超过大小限制。",
    };
    let message = messages[response.status] ?? "服务暂时不可用，请稍后重试。";
    // Native extractor text and proxy errors do not have the JSON domain-error shape.
    if (typeof error === "object" && error !== null) {
      if ("error" in error && error.error === "session_already_exists") {
        message = "该来源已有相同的 Session ID，请检查来源或对应会话。";
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
  if (data === undefined)
    throw new ApiError(response.status, "服务暂时不可用，请稍后重试。");
  return data;
}

// Keep the complete body typed before serialization; the browser owns Content-Type and boundary.
export function serializeImport(
  body: components["schemas"]["FileImport"],
): FormData {
  const form = new FormData();
  for (const [name, value] of Object.entries(body)) form.set(name, value);
  return form;
}

export const libraryOptions = {
  queryKey: ["conversations"],
  queryFn: () => apiData(api.GET("/api/conversations")),
};
export function formatTime(value: number) {
  return new Intl.DateTimeFormat("zh-CN", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(value);
}
