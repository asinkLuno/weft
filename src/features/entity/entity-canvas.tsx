import {
  Background,
  type Edge,
  Panel,
  ReactFlow,
  ReactFlowProvider,
  useNodesInitialized,
  useNodesState,
  useReactFlow,
} from "@xyflow/react";
import { Maximize, Minus, Plus } from "lucide-react";
import { useEffect, useMemo, useRef } from "react";
import type { Lang } from "@/i18n";
import { EntityNode, EntitySelectContext } from "./entity-card";
import { FloatingEdge } from "./floating-edge";
import {
  computeEntityLayout,
  DEFAULT_NODE_SIZE,
  type EntityLayoutMode,
  type NodeSize,
  type Point,
} from "./layout";
import type { EntityFlowNode } from "./model";

const nodeTypes = { entity: EntityNode };
const edgeTypes = { floating: FloatingEdge };

export type EntityCanvasProps = {
  initialNodes: EntityFlowNode[];
  edges: Edge[];
  layoutMode: EntityLayoutMode;
  savedPositions: Readonly<Record<string, Point>>;
  selectedId: string | null;
  focusTarget: { id: string; seq: number } | null;
  lang: Lang;
  onSelect: (id: string | null) => void;
  onJumpToEntity: (id: string) => void;
  onPersist?: (id: string, pos: Point) => void;
};

type MeasuredSnapshot = {
  sizes: Map<string, NodeSize>;
  ids: string[];
  fingerprint: string;
};

function collectMeasuredSnapshot(
  nodes: readonly EntityFlowNode[],
  layoutMode: EntityLayoutMode,
  edgeSignature: string,
): MeasuredSnapshot | null {
  if (nodes.length === 0) return null;
  const sizes = new Map<string, NodeSize>();
  const parts: string[] = [];
  for (const node of nodes) {
    const w = node.measured?.width;
    const h = node.measured?.height;
    if (!w || !h) return null;
    const rw = Math.ceil(w / 2) * 2;
    const rh = Math.ceil(h / 2) * 2;
    sizes.set(node.id, { width: rw, height: rh });
    parts.push(`${node.id}:${rw}x${rh}`);
  }
  return {
    sizes,
    ids: nodes.map((n) => n.id),
    fingerprint: `${layoutMode}|${edgeSignature}|${parts.join(",")}`,
  };
}

function mergeIncomingNodes(
  current: readonly EntityFlowNode[],
  incoming: readonly EntityFlowNode[],
  selectedId: string | null,
): EntityFlowNode[] {
  const previous = new Map(current.map((node) => [node.id, node]));
  return incoming.map((next) => {
    const old = previous.get(next.id);
    return old
      ? {
          ...old,
          ...next,
          position: old.position,
          measured: old.measured,
          selected: next.id === selectedId,
        }
      : { ...next, selected: next.id === selectedId };
  });
}

function applyComputedPositions(
  current: readonly EntityFlowNode[],
  computed: Readonly<Record<string, Point>>,
  layoutMode: EntityLayoutMode,
  saved: Readonly<Record<string, Point>>,
): EntityFlowNode[] {
  return current.map((node) => {
    if (layoutMode === "grid" && node.id in saved) return node;
    const nextPos = computed[node.id];
    if (!nextPos) return node;
    if (
      Math.abs(node.position.x - nextPos.x) < 1 &&
      Math.abs(node.position.y - nextPos.y) < 1
    ) {
      return node;
    }
    return { ...node, position: nextPos };
  });
}

export function EntityCanvas(props: EntityCanvasProps) {
  return (
    <ReactFlowProvider>
      <EntityCanvasInner {...props} />
    </ReactFlowProvider>
  );
}

function EntityCanvasInner({
  initialNodes,
  edges,
  layoutMode,
  savedPositions,
  selectedId,
  focusTarget,
  lang,
  onSelect,
  onJumpToEntity,
  onPersist,
}: EntityCanvasProps) {
  const [nodes, setNodes, onNodesChange] = useNodesState<EntityFlowNode>(
    initialNodes.map((n) => ({ ...n, selected: n.id === selectedId })),
  );
  const nodesInitialized = useNodesInitialized();
  const { fitView, setCenter, getZoom } = useReactFlow();

  const lastLayoutFingerprintRef = useRef<string | null>(null);
  const didInitialFitRef = useRef(false);
  const handledFocusSeqRef = useRef(0);
  const savedPositionsRef = useRef(savedPositions);
  savedPositionsRef.current = savedPositions;

  // 文档编辑或选中项变化时增量同步节点，保留已有位置与实测尺寸，不重置视口
  useEffect(() => {
    setNodes((current) =>
      mergeIncomingNodes(current, initialNodes, selectedId),
    );
  }, [initialNodes, selectedId, setNodes]);

  const edgeSignature = useMemo(
    () => edges.map((e) => `${e.source}->${e.target}`).join(","),
    [edges],
  );

  const measuredSnapshot = useMemo(
    () =>
      nodesInitialized
        ? collectMeasuredSnapshot(nodes, layoutMode, edgeSignature)
        : null,
    [nodesInitialized, nodes, layoutMode, edgeSignature],
  );

  // 测得真实宽高后按实测尺寸精准重排；只在首次测量后 fitView，后续编辑不跳视角
  useEffect(() => {
    if (
      !measuredSnapshot ||
      measuredSnapshot.fingerprint === lastLayoutFingerprintRef.current
    ) {
      return;
    }
    lastLayoutFingerprintRef.current = measuredSnapshot.fingerprint;

    const computed = computeEntityLayout(
      layoutMode,
      measuredSnapshot.ids,
      edges,
      measuredSnapshot.sizes,
    );
    setNodes((current) =>
      applyComputedPositions(
        current,
        computed,
        layoutMode,
        savedPositionsRef.current,
      ),
    );

    if (!didInitialFitRef.current) {
      didInitialFitRef.current = true;
      requestAnimationFrame(() => void fitView({ padding: 0.12 }));
    }
  }, [measuredSnapshot, layoutMode, edges, setNodes, fitView]);

  // 详情栏或父级 badge 点击跳转实体时，若目标在画布内则平滑居中
  useEffect(() => {
    if (!focusTarget || focusTarget.seq === handledFocusSeqRef.current) return;
    const target = nodes.find((n) => n.id === focusTarget.id);
    if (!target) return;
    handledFocusSeqRef.current = focusTarget.seq;
    const w = target.measured?.width ?? DEFAULT_NODE_SIZE.width;
    const h = target.measured?.height ?? DEFAULT_NODE_SIZE.height;
    void setCenter(target.position.x + w / 2, target.position.y + h / 2, {
      zoom: Math.max(getZoom(), 0.85),
      duration: 240,
    });
  }, [focusTarget, nodes, setCenter, getZoom]);

  return (
    <EntitySelectContext.Provider value={onJumpToEntity}>
      <ReactFlow
        nodes={nodes}
        edges={edges}
        onNodesChange={onNodesChange}
        nodeTypes={nodeTypes}
        edgeTypes={edgeTypes}
        onNodeClick={(_, node) => onSelect(node.id)}
        onPaneClick={() => onSelect(null)}
        onNodeDragStop={
          onPersist ? (_, node) => onPersist(node.id, node.position) : undefined
        }
        minZoom={0.1}
      >
        <Background gap={24} color="var(--color-base-300)" />
        <CanvasControls lang={lang} />
      </ReactFlow>
    </EntitySelectContext.Provider>
  );
}

function CanvasControls({ lang }: { lang: Lang }) {
  const { zoomIn, zoomOut, fitView } = useReactFlow();
  const zh = lang === "zh";
  return (
    <Panel position="bottom-left" className="m-2!">
      <div className="join">
        <button
          type="button"
          className="btn btn-square btn-sm join-item border-base-300 bg-base-100"
          aria-label={zh ? "放大" : "Zoom in"}
          onClick={() => zoomIn()}
        >
          <Plus className="size-4" />
        </button>
        <button
          type="button"
          className="btn btn-square btn-sm join-item border-base-300 bg-base-100"
          aria-label={zh ? "缩小" : "Zoom out"}
          onClick={() => zoomOut()}
        >
          <Minus className="size-4" />
        </button>
        <button
          type="button"
          className="btn btn-square btn-sm join-item border-base-300 bg-base-100"
          aria-label={zh ? "适应画布" : "Fit view"}
          onClick={() => fitView({ padding: 0.12 })}
        >
          <Maximize className="size-4" />
        </button>
      </div>
    </Panel>
  );
}
