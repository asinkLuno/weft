import {
  Background,
  type DefaultEdgeOptions,
  type Edge,
  Panel,
  ReactFlow,
  ReactFlowProvider,
  useNodesInitialized,
  useNodesState,
  useReactFlow,
} from "@xyflow/react";
import { Maximize, Minus, Plus } from "lucide-react";
import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
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
const DEFAULT_EDGE_OPTIONS: DefaultEdgeOptions = {
  style: {
    stroke: "var(--color-primary)",
    strokeWidth: 1.5,
  },
};

const ZOOM_IN_ICON = <Plus className="size-4" />;
const ZOOM_OUT_ICON = <Minus className="size-4" />;
const FIT_VIEW_ICON = <Maximize className="size-4" />;

export type EntityCanvasProps = {
  initialNodes: EntityFlowNode[];
  edges: Edge[];
  layoutMode: EntityLayoutMode;
  getSavedPositions: () => Readonly<Record<string, Point>>;
  selectedId: string | null;
  focusTarget: { id: string; seq: number } | null;
  lang: Lang;
  onSelect: (id: string | null) => void;
  onJumpToEntity: (id: string) => void;
  onPersist?: (id: string, pos: Point) => void;
};

function computeMeasuredFingerprint(
  nodes: readonly EntityFlowNode[],
  layoutMode: EntityLayoutMode,
  edgeSignature: string,
): string | null {
  if (nodes.length === 0) return null;
  let fingerprint = `${layoutMode}|${edgeSignature}|`;
  for (let i = 0; i < nodes.length; i++) {
    const node = nodes[i];
    const w = node.measured?.width;
    const h = node.measured?.height;
    if (!w || !h) return null;
    const rw = Math.ceil(w / 2) * 2;
    const rh = Math.ceil(h / 2) * 2;
    if (i > 0) fingerprint += ",";
    fingerprint += `${node.id}:${rw}x${rh}`;
  }
  return fingerprint;
}

function extractMeasuredLayoutInput(nodes: readonly EntityFlowNode[]): {
  sizes: Map<string, NodeSize>;
  ids: string[];
} {
  const sizes = new Map<string, NodeSize>();
  const ids: string[] = new Array(nodes.length);
  for (let i = 0; i < nodes.length; i++) {
    const node = nodes[i];
    const w = node.measured?.width ?? DEFAULT_NODE_SIZE.width;
    const h = node.measured?.height ?? DEFAULT_NODE_SIZE.height;
    sizes.set(node.id, {
      width: Math.ceil(w / 2) * 2,
      height: Math.ceil(h / 2) * 2,
    });
    ids[i] = node.id;
  }
  return { sizes, ids };
}

function mergeIncomingNodes(
  current: readonly EntityFlowNode[],
  incoming: readonly EntityFlowNode[],
  selectedId: string | null,
): EntityFlowNode[] {
  const previous = new Map(current.map((node) => [node.id, node]));
  let changed = current.length !== incoming.length;
  const merged = incoming.map((next, idx) => {
    const nextSelected = next.id === selectedId;
    const old = previous.get(next.id);
    if (!old) {
      changed = true;
      return { ...next, selected: nextSelected };
    }
    if (
      old.data === next.data &&
      old.type === next.type &&
      Boolean(old.selected) === nextSelected
    ) {
      if (current[idx] !== old) changed = true;
      return old;
    }
    changed = true;
    return {
      ...old,
      ...next,
      position: old.position,
      measured: old.measured,
      selected: nextSelected,
    };
  });
  return changed ? merged : (current as EntityFlowNode[]);
}

function applyComputedPositions(
  current: readonly EntityFlowNode[],
  computed: Readonly<Record<string, Point>>,
  layoutMode: EntityLayoutMode,
  saved: Readonly<Record<string, Point>>,
): EntityFlowNode[] {
  let changed = false;
  const nextNodes = current.map((node) => {
    if (layoutMode === "grid" && node.id in saved) return node;
    const nextPos = computed[node.id];
    if (!nextPos) return node;
    if (
      Math.abs(node.position.x - nextPos.x) < 1 &&
      Math.abs(node.position.y - nextPos.y) < 1
    ) {
      return node;
    }
    changed = true;
    return { ...node, position: nextPos };
  });
  return changed ? nextNodes : (current as EntityFlowNode[]);
}

export const EntityCanvas = memo(function EntityCanvas(
  props: EntityCanvasProps,
) {
  return (
    <ReactFlowProvider>
      <EntityCanvasInner {...props} />
    </ReactFlowProvider>
  );
});

function EntityCanvasInner({
  initialNodes,
  edges,
  layoutMode,
  getSavedPositions,
  selectedId,
  focusTarget,
  lang,
  onSelect,
  onJumpToEntity,
  onPersist,
}: EntityCanvasProps) {
  const [seedNodes] = useState<EntityFlowNode[]>(() =>
    initialNodes.map((n) => ({ ...n, selected: n.id === selectedId })),
  );
  const [nodes, setNodes, onNodesChange] =
    useNodesState<EntityFlowNode>(seedNodes);
  const nodesInitialized = useNodesInitialized();
  const { fitView, setCenter, getZoom, getNode } = useReactFlow();

  const lastLayoutFingerprintRef = useRef<string | null>(null);
  const didInitialFitRef = useRef(false);
  const handledFocusSeqRef = useRef(0);
  const nodesRef = useRef(nodes);

  useEffect(() => {
    nodesRef.current = nodes;
  }, [nodes]);

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

  // 派生原始字符串指纹：拖拽期间宽高不变则指纹不变，避免每帧触发重排 effect 与 Map 分配
  const measuredFingerprint = useMemo(
    () =>
      nodesInitialized
        ? computeMeasuredFingerprint(nodes, layoutMode, edgeSignature)
        : null,
    [nodesInitialized, nodes, layoutMode, edgeSignature],
  );

  // 测得真实宽高后按实测尺寸精准重排；只在首次测量后 fitView，后续编辑不跳视角
  useEffect(() => {
    if (
      !measuredFingerprint ||
      measuredFingerprint === lastLayoutFingerprintRef.current
    ) {
      return;
    }
    lastLayoutFingerprintRef.current = measuredFingerprint;

    const { sizes, ids } = extractMeasuredLayoutInput(nodesRef.current);
    const computed = computeEntityLayout(layoutMode, ids, edges, sizes);
    setNodes((current) =>
      applyComputedPositions(
        current,
        computed,
        layoutMode,
        getSavedPositions(),
      ),
    );

    if (!didInitialFitRef.current) {
      didInitialFitRef.current = true;
      requestAnimationFrame(() => void fitView({ padding: 0.12 }));
    }
  }, [
    measuredFingerprint,
    layoutMode,
    edges,
    setNodes,
    fitView,
    getSavedPositions,
  ]);

  // 详情栏或父级 badge 点击跳转实体时，若目标在画布内则平滑居中
  useEffect(() => {
    if (!focusTarget || focusTarget.seq === handledFocusSeqRef.current) return;
    const target =
      getNode(focusTarget.id) ??
      nodesRef.current.find((n) => n.id === focusTarget.id);
    if (!target) return;
    handledFocusSeqRef.current = focusTarget.seq;
    const w = target.measured?.width ?? DEFAULT_NODE_SIZE.width;
    const h = target.measured?.height ?? DEFAULT_NODE_SIZE.height;
    void setCenter(target.position.x + w / 2, target.position.y + h / 2, {
      zoom: Math.max(getZoom(), 0.85),
      duration: 240,
    });
  }, [focusTarget, getNode, setCenter, getZoom]);

  const handleNodeClick = useCallback(
    (_: unknown, node: EntityFlowNode) => onSelect(node.id),
    [onSelect],
  );

  const handlePaneClick = useCallback(() => onSelect(null), [onSelect]);

  const handleNodeDragStop = useCallback(
    (_: unknown, node: EntityFlowNode) => {
      onPersist?.(node.id, node.position);
    },
    [onPersist],
  );

  return (
    <EntitySelectContext.Provider value={onJumpToEntity}>
      <ReactFlow
        nodes={nodes}
        edges={edges}
        onNodesChange={onNodesChange}
        nodeTypes={nodeTypes}
        edgeTypes={edgeTypes}
        defaultEdgeOptions={DEFAULT_EDGE_OPTIONS}
        onNodeClick={handleNodeClick}
        onPaneClick={handlePaneClick}
        onNodeDragStop={onPersist ? handleNodeDragStop : undefined}
        minZoom={0.1}
      >
        <Background gap={24} color="var(--color-base-300)" />
        <CanvasControls lang={lang} />
      </ReactFlow>
    </EntitySelectContext.Provider>
  );
}

const CanvasControls = memo(function CanvasControls({ lang }: { lang: Lang }) {
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
          {ZOOM_IN_ICON}
        </button>
        <button
          type="button"
          className="btn btn-square btn-sm join-item border-base-300 bg-base-100"
          aria-label={zh ? "缩小" : "Zoom out"}
          onClick={() => zoomOut()}
        >
          {ZOOM_OUT_ICON}
        </button>
        <button
          type="button"
          className="btn btn-square btn-sm join-item border-base-300 bg-base-100"
          aria-label={zh ? "适应画布" : "Fit view"}
          onClick={() => fitView({ padding: 0.12 })}
        >
          {FIT_VIEW_ICON}
        </button>
      </div>
    </Panel>
  );
});
