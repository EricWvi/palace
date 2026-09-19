import { expect, it } from "vitest";
import { layoutBranches } from "./branch-layout";
import { userTree, type ResolvedPath } from "./conversation-tree";
import type { ConversationPath, Message } from "./api";

function path(id: string): ConversationPath {
  return {
    id,
    session_id: id,
    head_message_id: id,
    message_count: 1,
    occurred_at: 0,
    created_at: 0,
    updated_at: 0,
    original_link: id,
  };
}
function message(id: string, role: Message["role"]): Message {
  return { id, role, content: id, parent_message_id: null, created_order: 0 };
}

it("projects through assistant nodes and lays out parents above non-overlapping siblings", () => {
  const root = message("root", "user");
  const answer = message("answer", "assistant");
  const paths: ResolvedPath[] = [
    { path: path("left"), messages: [root, answer, message("left", "user")] },
    { path: path("right"), messages: [root, answer, message("right", "user")] },
    { path: path("short"), messages: [root, answer] },
    { path: path("same"), messages: [root, answer] },
  ];
  const tree = userTree(paths);
  expect(
    tree.nodes.map(({ message, parentId, paths }) => [
      message.id,
      parentId,
      paths.map((p) => p.id),
    ]),
  ).toEqual([
    ["root", null, ["short", "same"]],
    ["left", "root", ["left"]],
    ["right", "root", ["right"]],
  ]);
  const [top, left, right] = layoutBranches(tree);
  expect(top.position.y + top.height).toBeLessThan(left.position.y);
  expect(top.position.y + top.height).toBeLessThan(right.position.y);
  expect(Math.abs(left.position.x - right.position.x)).toBeGreaterThanOrEqual(
    left.width,
  );
  expect(layoutBranches(tree)).toEqual([top, left, right]);
});

it("keeps assistant-only endpoints manageable and accepts an empty tree", () => {
  const endpoint = path("assistant-only");
  const cards = layoutBranches(
    userTree([{ path: endpoint, messages: [message("answer", "assistant")] }]),
  );
  expect(cards).toEqual([
    {
      id: "without-user",
      parentId: null,
      title: "无用户消息",
      paths: [endpoint],
      width: 260,
      height: 126,
      position: { x: 24, y: 24 },
    },
  ]);
  expect(layoutBranches({ nodes: [], withoutUser: [] })).toEqual([]);
});
