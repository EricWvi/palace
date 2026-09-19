import { createContext, useContext, useMemo } from "react";
import {
  Background,
  BackgroundVariant,
  Controls,
  Handle,
  Position,
  ReactFlow,
  type Node,
  type NodeProps,
} from "@xyflow/react";
import { GitBranch, MessageSquare, Pencil, Trash2 } from "lucide-react";
import type { ConversationPath } from "@/lib/api";
import { layoutBranches } from "@/lib/branch-layout";
import type { userTree } from "@/lib/conversation-tree";
import { Button } from "./ui/button";
import "@xyflow/react/dist/style.css";
import "./branch-graph.css";

interface Actions {
  onUpdate: (path: ConversationPath) => void;
  onDelete: (path: ConversationPath) => void;
  canDelete: boolean;
}
const ActionsContext = createContext<Actions | null>(null);
type BranchNode = Node<
  { title: string; paths: ConversationPath[]; root: boolean },
  "branch"
>;

function BranchNodeCard({ data }: NodeProps<BranchNode>) {
  const actions = useContext(ActionsContext)!;
  return (
    <div className={`branch-node-card${data.paths.length ? " has-paths" : ""}`}>
      {!data.root && <Handle type="target" position={Position.Top} />}
      <div className="branch-node-heading">
        <MessageSquare size={14} />
        <span>{data.root ? "对话起点" : "用户消息"}</span>
        {data.paths.length > 0 && (
          <span className="branch-node-count">
            <GitBranch size={12} />
            {data.paths.length}
          </span>
        )}
      </div>
      <p className="branch-node-text" title={data.title}>
        {data.title}
      </p>
      {data.paths.length > 0 && (
        <div className="branch-node-sessions">
          {data.paths.map((path) => (
            <div key={path.id} className="path-actions">
              <span className="path-session" title={path.session_id}>
                {path.session_id}
              </span>
              <div className="branch-node-buttons nodrag nopan">
                <Button
                  variant="ghost"
                  size="icon"
                  title="更新分支"
                  aria-label={`更新分支 ${path.session_id}`}
                  onClick={() => actions.onUpdate(path)}
                >
                  <Pencil size={14} />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  title={
                    actions.canDelete
                      ? "删除分支"
                      : "最后一个分支请通过删除对话移除"
                  }
                  aria-label={`删除分支 ${path.session_id}`}
                  disabled={!actions.canDelete}
                  onClick={() => actions.onDelete(path)}
                >
                  <Trash2 size={14} />
                </Button>
              </div>
            </div>
          ))}
        </div>
      )}
      <Handle type="source" position={Position.Bottom} />
    </div>
  );
}
const nodeTypes = { branch: BranchNodeCard };
const ariaLabelConfig = {
  "controls.zoomIn.ariaLabel": "放大树图",
  "controls.zoomOut.ariaLabel": "缩小树图",
  "controls.fitView.ariaLabel": "适应画布",
};

export function BranchGraph({
  tree,
  onUpdate,
  onDelete,
  canDelete,
}: Actions & { tree: ReturnType<typeof userTree> }) {
  const { nodes, edges } = useMemo(() => {
    const cards = layoutBranches(tree);
    return {
      nodes: cards.map((card): BranchNode => ({
        id: card.id,
        type: "branch",
        position: card.position,
        width: card.width,
        height: card.height,
        // Read-only positioning must not disable hover and embedded action buttons.
        style: { pointerEvents: "all" },
        data: { title: card.title, paths: card.paths, root: !card.parentId },
      })),
      edges: cards
        .filter((card) => card.parentId)
        .map((card) => ({
          id: `${card.parentId}-${card.id}`,
          source: card.parentId!,
          target: card.id,
          type: "default",
        })),
    };
  }, [tree]);
  return (
    <ActionsContext.Provider value={{ onUpdate, onDelete, canDelete }}>
      <div className="branch-graph" aria-label="用户消息树">
        <ReactFlow
          nodes={nodes}
          edges={edges}
          nodeTypes={nodeTypes}
          fitView
          fitViewOptions={{ padding: 0.18, maxZoom: 1 }}
          minZoom={0.2}
          maxZoom={1.8}
          nodesDraggable={false}
          nodesConnectable={false}
          elementsSelectable={false}
          nodesFocusable={false}
          edgesFocusable={false}
          deleteKeyCode={null}
          ariaLabelConfig={ariaLabelConfig}
        >
          <Background
            variant={BackgroundVariant.Dots}
            gap={20}
            size={1}
            color="#cdd4c4"
          />
          <Controls showInteractive={false} position="bottom-left" />
        </ReactFlow>
      </div>
    </ActionsContext.Provider>
  );
}
