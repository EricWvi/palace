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
}
export interface Summary extends Conversation {
  session_ids: string[];
  path_count: number;
  path_id: string;
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
  paths: ConversationPath[];
}
export interface ConversationPath {
  id: string;
  session_id: string;
  head_message_id: string;
  message_count: number;
  occurred_at: number;
  created_at: number;
  updated_at: number;
  original_link: string;
}
export interface ImportResult {
  conversation_id: string;
  path_id: string;
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
      body.error === "session_already_exists"
        ? "该来源的 Session ID 已存在，请在对应会话的分支管理中更新。"
        : body.path
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
