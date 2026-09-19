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

export interface UserNode {
  message: Message;
  parentId: string | null;
  paths: ConversationPath[];
}

export interface BranchChoice {
  key: string;
  label: string;
  pathId: string;
}

// A continuation selects the newest complete path in that subtree; each endpoint keeps its session.
export function branchChoices(
  paths: ResolvedPath[],
  selected: ResolvedPath,
  after: number,
): BranchChoice[] {
  const choices = new Map<string, BranchChoice>();
  for (const candidate of paths) {
    // A message has exactly one parent, so matching this node proves the entire prefix.
    if (candidate.messages[after]?.id !== selected.messages[after]?.id)
      continue;
    const next = candidate.messages[after + 1];
    const key = next ? `message:${next.id}` : `path:${candidate.path.id}`;
    if (!choices.has(key))
      choices.set(key, {
        key,
        label: next
          ? `${next.role === "user" ? "你" : "回答"}：${next.content.slice(0, 60) || "（空消息）"}`
          : `在此结束 · ${candidate.path.session_id}`,
        pathId: candidate.path.id,
      });
  }
  return [...choices.values()];
}

// Project each endpoint onto its last user ancestor, including endpoints inside longer paths.
export function userTree(paths: ResolvedPath[]): {
  nodes: UserNode[];
  withoutUser: ConversationPath[];
} {
  const nodes = new Map<string, UserNode>();
  const children = new Map<string | null, string[]>();
  const withoutUser: ConversationPath[] = [];
  for (const { path, messages } of paths) {
    let parent: string | null = null;
    for (const message of messages) {
      if (message.role !== "user") continue;
      if (!nodes.has(message.id)) {
        nodes.set(message.id, { message, parentId: parent, paths: [] });
        children.set(parent, [...(children.get(parent) ?? []), message.id]);
      }
      parent = message.id;
    }
    if (parent) nodes.get(parent)!.paths.push(path);
    else withoutUser.push(path);
  }
  const ordered: UserNode[] = [];
  const stack = [...(children.get(null) ?? [])].reverse();
  while (stack.length) {
    const id = stack.pop()!;
    ordered.push(nodes.get(id)!);
    stack.push(...[...(children.get(id) ?? [])].reverse());
  }
  return { nodes: ordered, withoutUser };
}
