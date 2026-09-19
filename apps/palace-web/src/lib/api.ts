export type Source = "chatgpt" | "gemini" | "grok";
export const sources: Record<Source, string> = {
  chatgpt: "ChatGPT",
  gemini: "Gemini",
  grok: "Grok",
};
export interface Conversation {
  id: string;
  title: string;
  source: Source;
  session_id: string;
}
export interface Summary extends Conversation {
  occurred_at: number;
  head_message_id: string;
  message_count: number;
}
export interface Message {
  id: string;
  parent_message_id: string | null;
  role: "user" | "assistant";
  content: string;
  created_order: number;
}
export interface Detail {
  conversation: Conversation;
  messages: Message[];
  original_link: string;
}
export interface ImportResult {
  conversation_id: string;
  head_message_id: string;
}
export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
  }
}
export async function request<T>(
  path: string,
  options?: RequestInit,
): Promise<T> {
  const response = await fetch(path, {
    credentials: "same-origin",
    ...options,
  });
  if (!response.ok) {
    const body = await response.json().catch(() => ({}));
    const messages: Record<number, string> = {
      401: "请先登录后再继续。",
      403: "请求来源未被允许，请检查开发环境配置。",
      404: "找不到这段会话。",
      409: "导入请求冲突，请重新打开导入窗口。",
      413: "文件或消息超过大小限制。",
    };
    throw new ApiError(
      response.status,
      body.path
        ? `${body.path}：${body.message}`
        : (messages[response.status] ?? "服务暂时不可用，请稍后重试。"),
    );
  }
  return response.json() as Promise<T>;
}
export const libraryOptions = {
  queryKey: ["conversations"],
  queryFn: () => request<Summary[]>("/api/conversations"),
};
export function formatTime(value: number) {
  return new Intl.DateTimeFormat("zh-CN", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(value);
}
