import { graphlib, layout } from "@dagrejs/dagre";
import type { userTree } from "./conversation-tree";
import type { ConversationPath } from "./api";

export interface BranchCard {
  id: string;
  parentId: string | null;
  title: string;
  paths: ConversationPath[];
  width: number;
  height: number;
  position: { x: number; y: number };
}

// Reserve action space before layout so hover never moves nodes or their connecting edges.
export function layoutBranches(
  tree: ReturnType<typeof userTree>,
): BranchCard[] {
  const cards = tree.nodes.map(({ message, parentId, paths }) => ({
    id: message.id,
    parentId,
    title: message.content || "（空消息）",
    paths,
  }));
  if (tree.withoutUser.length)
    cards.push({
      id: "without-user",
      parentId: null,
      title: "无用户消息",
      paths: tree.withoutUser,
    });
  const graph = new graphlib.Graph();
  graph.setGraph({
    rankdir: "TB",
    nodesep: 48,
    ranksep: 64,
    marginx: 24,
    marginy: 24,
  });
  graph.setDefaultEdgeLabel(() => ({}));
  for (const card of cards)
    graph.setNode(card.id, { width: 260, height: 88 + card.paths.length * 38 });
  for (const card of cards)
    if (card.parentId) graph.setEdge(card.parentId, card.id);
  layout(graph);
  return cards.map((card) => {
    const { x, y, width, height } = graph.node(card.id);
    return {
      ...card,
      width,
      height,
      position: { x: x - width / 2, y: y - height / 2 },
    };
  });
}
