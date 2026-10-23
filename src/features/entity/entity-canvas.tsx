import {
  Background,
  type Edge,
  Panel,
  ReactFlow,
  useNodesState,
  useReactFlow,
} from "@xyflow/react";
import { Maximize, Minus, Plus } from "lucide-react";
import { useEffect } from "react";
import { Button } from "@/components/ui/button";
import type { Lang } from "@/i18n";
import { EntityNode } from "./entity-card";
import type { EntityFlowNode } from "./model";

const nodeTypes = { entity: EntityNode };
export function EntityCanvas({
  initialNodes,
  edges,
  lang,
  onPersist,
}: {
  initialNodes: EntityFlowNode[];
  lang: Lang;
  edges: Edge[];
  // 不传则拖动不落盘（筛选视图的临时摆放）
  onPersist?: (id: string, pos: { x: number; y: number }) => void;
}) {
  const [nodes, setNodes, onNodesChange] = useNodesState(initialNodes);
  // 筛选图采用新布局；自由画布保留已有节点的拖放位置。
  useEffect(() => {
    setNodes((current) => {
      const previous = new Map(current.map((node) => [node.id, node]));
      return initialNodes.map((next) => {
        const old = previous.get(next.id);
        return old
          ? {
              ...old,
              ...next,
              position: onPersist ? old.position : next.position,
            }
          : next;
      });
    });
  }, [initialNodes, onPersist, setNodes]);
  return (
    <ReactFlow
      nodes={nodes}
      edges={edges}
      onNodesChange={onNodesChange}
      nodeTypes={nodeTypes}
      onNodeDragStop={
        onPersist ? (_, node) => onPersist(node.id, node.position) : undefined
      }
      fitView
      minZoom={0.1}
    >
      <Background gap={24} color="var(--border)" />
      <CanvasControls lang={lang} />
    </ReactFlow>
  );
}

function CanvasControls({ lang }: { lang: Lang }) {
  const { zoomIn, zoomOut, fitView } = useReactFlow();
  const zh = lang === "zh";
  return (
    <Panel position="bottom-left" className="canvas-controls">
      <Button
        variant="ghost"
        size="icon"
        aria-label={zh ? "放大" : "Zoom in"}
        onClick={() => zoomIn()}
      >
        <Plus />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        aria-label={zh ? "缩小" : "Zoom out"}
        onClick={() => zoomOut()}
      >
        <Minus />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        aria-label={zh ? "适应画布" : "Fit view"}
        onClick={() => fitView()}
      >
        <Maximize />
      </Button>
    </Panel>
  );
}
