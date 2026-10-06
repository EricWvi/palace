import { expect, it } from "vitest";
import type { ConversationPath, Message } from "./api";
import { branchChoices, type ResolvedPath } from "./conversation-tree";

// Paths given as chains of message ids, newest first as resolvePaths orders them.
function paths(
  ...chains: [id: string, title: string, ids: string[]][]
): ResolvedPath[] {
  return chains.map(([id, title, ids]) => ({
    path: { id, title, session_id: `s-${id}` } as ConversationPath,
    messages: ids.map((message) => ({ id: message }) as Message),
  }));
}

// Core test case: `specs/test-cases/server/conversation/message-tree.md#fork-selection-must-resolve-to-one-real-source-session`
it("names fork options by path title, telling same-named branches apart by session", () => {
  const tree = paths(
    ["p3", "计划周末出行", ["U1", "A1", "U3", "A3"]],
    ["p2", "计划周末出行", ["U1", "A1", "U3"]],
    ["p1", "计划周末出行", ["U1", "A1", "U2"]],
    ["p0", "很长很长很长很长很长很长的分支标题", ["U1", "A1", "U4"]],
  );
  // The option holding the path being read is named after it, not after the newest path in that
  // subtree, which is what picking the option from another branch opens.
  expect(branchChoices(tree, tree[1], 1)).toEqual([
    { key: "message:U3", pathId: "p3", label: "计划周末出行 · s-p2" },
    { key: "message:U2", pathId: "p1", label: "计划周末出行 · s-p1" },
    { key: "message:U4", pathId: "p0", label: "很长很长很长很长很长很长的分…" },
  ]);
  // An internal endpoint is named like any other branch.
  expect(branchChoices(tree, tree[0], 2)).toEqual([
    { key: "message:A3", pathId: "p3", label: "计划周末出行 · s-p3" },
    { key: "path:p2", pathId: "p2", label: "计划周末出行 · s-p2" },
  ]);
});
