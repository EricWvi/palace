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
  depth: number;
  paths: ConversationPath[];
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
    let depth = 0;
    for (const message of messages) {
      if (message.role !== "user") continue;
      if (!nodes.has(message.id)) {
        nodes.set(message.id, { message, depth, paths: [] });
        children.set(parent, [...(children.get(parent) ?? []), message.id]);
      }
      parent = message.id;
      depth++;
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
