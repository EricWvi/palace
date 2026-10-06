import type { ConversationPath, Detail, Message } from "./api";

export interface ResolvedPath {
  path: ConversationPath;
  messages: Message[];
}

// Build only real paths; selectors and session links must never combine unrelated suffixes.
export function resolvePaths(detail: Detail): ResolvedPath[] {
  const index = new Map(
    detail.messages.map((message) => [message.id, message]),
  );
  return [...detail.paths]
    .sort((a, b) => b.updated_at - a.updated_at || b.id.localeCompare(a.id))
    .map((path) => {
      const messages: Message[] = [];
      const visited = new Set<string>();
      let id: string | null = path.head_message_id;
      while (id) {
        const message = index.get(id);
        if (!message || visited.has(id))
          throw new Error("对话路径不完整，请重新加载。");
        visited.add(id);
        messages.push(message);
        id = message.parent_message_id;
      }
      return { path, messages: messages.reverse() };
    });
}

export interface BranchChoice {
  key: string;
  label: string;
  pathId: string;
}

// A fork option is named by the first words of its branch, the way a reader remembers it.
const LABEL_CHARS = 14;
function optionLabel(content: string): string {
  // Markdown markers would eat into the few characters shown; they never read as words.
  const text = content
    .replace(/[#*_`>~]/g, "")
    .replace(/\s+/g, " ")
    .trim();
  if (!text) return "（空消息）";
  const chars = [...text];
  return chars.length > LABEL_CHARS
    ? `${chars.slice(0, LABEL_CHARS).join("")}…`
    : text;
}

// A continuation selects the newest complete path in that subtree; each endpoint keeps its session.
export function branchChoices(
  paths: ResolvedPath[],
  selected: ResolvedPath,
  after: number,
): BranchChoice[] {
  const choices = new Map<string, BranchChoice>();
  const endings: ConversationPath[] = [];
  for (const candidate of paths) {
    // A message has exactly one parent, so matching this node proves the entire prefix.
    if (candidate.messages[after]?.id !== selected.messages[after]?.id)
      continue;
    const next = candidate.messages[after + 1];
    if (!next) endings.push(candidate.path);
    const key = next ? `message:${next.id}` : `path:${candidate.path.id}`;
    if (!choices.has(key))
      choices.set(key, {
        key,
        label: next ? optionLabel(next.content) : "",
        pathId: candidate.path.id,
      });
  }
  // Several sessions may stop at the same message; only then does the session tell them apart.
  for (const path of endings)
    choices.get(`path:${path.id}`)!.label =
      endings.length > 1 ? `在此结束 · ${path.session_id}` : "在此结束";
  return [...choices.values()];
}
