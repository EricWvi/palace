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

// A fork option is named by the title of the path it opens, the name the reader gave it.
const LABEL_CHARS = 14;
function titleLabel(title: string): string {
  const chars = [...title.trim()];
  return chars.length > LABEL_CHARS
    ? `${chars.slice(0, LABEL_CHARS).join("")}…`
    : chars.join("");
}

// A continuation selects the newest complete path in that subtree, or the path being read when it
// lies there; an internal endpoint is the session that stops at it. Each option is named by that
// path's title, and the session tells apart options that would otherwise read the same, as new
// branches start with the title of the path they were made from.
export function branchChoices(
  paths: ResolvedPath[],
  selected: ResolvedPath,
  after: number,
): BranchChoice[] {
  // Per option: the path it opens, and the path it is named after (the same, unless the reader
  // is already inside that subtree on an older path).
  const options = new Map<
    string,
    { opens: ConversationPath; named: ConversationPath }
  >();
  for (const candidate of paths) {
    // A message has exactly one parent, so matching this node proves the entire prefix.
    if (candidate.messages[after]?.id !== selected.messages[after]?.id)
      continue;
    const next = candidate.messages[after + 1];
    const key = next ? `message:${next.id}` : `path:${candidate.path.id}`;
    const known = options.get(key);
    if (!known)
      options.set(key, { opens: candidate.path, named: candidate.path });
    else if (candidate.path.id === selected.path.id)
      known.named = candidate.path;
  }
  const entries = [...options];
  const labels = entries.map(([, { named }]) => titleLabel(named.title));
  return entries.map(([key, { opens, named }], index) => {
    const label = labels[index];
    const shared = labels.filter((other) => other === label).length > 1;
    return {
      key,
      pathId: opens.id,
      label:
        shared || !label
          ? [label, named.session_id].filter(Boolean).join(" · ")
          : label,
    };
  });
}
