import {
  BaseEdge,
  EdgeLabelRenderer,
  type EdgeProps,
  type InternalNode,
  useInternalNode,
} from "@xyflow/react";
import { memo } from "react";
import { DEFAULT_NODE_SIZE } from "./layout";
import type { FloatingEdgeData } from "./model";

const FLOATING_EDGE_STYLE = {
  stroke: "var(--color-primary)",
  strokeWidth: 1.5,
} as const;

const round1 = (value: number) => Math.round(value * 10) / 10;

type Rect = { x: number; y: number; w: number; h: number };

function nodeRect(node: InternalNode): Rect {
  const w = node.measured.width ?? DEFAULT_NODE_SIZE.width;
  const h = node.measured.height ?? DEFAULT_NODE_SIZE.height;
  const { x, y } = node.internals.positionAbsolute;
  return { x, y, w, h };
}

/** 计算从矩形中心指向 (tx, ty) 的射线与矩形边界的交点。 */
function borderPoint(rect: Rect, tx: number, ty: number) {
  const cx = rect.x + rect.w / 2;
  const cy = rect.y + rect.h / 2;
  const dx = tx - cx;
  const dy = ty - cy;
  if (dx === 0 && dy === 0) return { x: cx, y: cy };

  const scaleX =
    dx !== 0 ? rect.w / 2 / Math.abs(dx) : Number.POSITIVE_INFINITY;
  const scaleY =
    dy !== 0 ? rect.h / 2 / Math.abs(dy) : Number.POSITIVE_INFINITY;
  const t = Math.min(scaleX, scaleY);
  return { x: cx + dx * t, y: cy + dy * t };
}

function computeFloatingPath(
  sRect: Rect,
  tRect: Rect,
  curveOffset: number,
  canonicalForward: boolean,
) {
  const sCx = sRect.x + sRect.w / 2;
  const sCy = sRect.y + sRect.h / 2;
  const tCx = tRect.x + tRect.w / 2;
  const tCy = tRect.y + tRect.h / 2;

  const dx = tCx - sCx;
  const dy = tCy - sCy;
  const len = Math.hypot(dx, dy) || 1;
  const sign = canonicalForward ? 1 : -1;
  const nx = (-dy / len) * sign;
  const ny = (dx / len) * sign;

  const aimX = (sCx + tCx) / 2 + nx * curveOffset;
  const aimY = (sCy + tCy) / 2 + ny * curveOffset;
  const start = borderPoint(sRect, aimX, aimY);
  const end = borderPoint(tRect, aimX, aimY);

  const sx = round1(start.x);
  const sy = round1(start.y);
  const ex = round1(end.x);
  const ey = round1(end.y);

  if (curveOffset === 0) {
    return {
      path: `M ${sx} ${sy} L ${ex} ${ey}`,
      labelX: round1((sx + ex) / 2),
      labelY: round1((sy + ey) / 2),
    };
  }

  const ctrlX = round1((sx + ex) / 2 + nx * curveOffset);
  const ctrlY = round1((sy + ey) / 2 + ny * curveOffset);
  return {
    path: `M ${sx} ${sy} Q ${ctrlX} ${ctrlY} ${ex} ${ey}`,
    labelX: round1(0.25 * sx + 0.5 * ctrlX + 0.25 * ex),
    labelY: round1(0.25 * sy + 0.5 * ctrlY + 0.25 * ey),
  };
}

export const FloatingEdge = memo(function FloatingEdge({
  id,
  source,
  target,
  data,
}: EdgeProps) {
  const sourceNode = useInternalNode(source);
  const targetNode = useInternalNode(target);
  if (!sourceNode || !targetNode) return null;

  const edgeData = data as FloatingEdgeData | undefined;
  const pairTotal = edgeData?.pairTotal ?? 1;
  const pairIndex = edgeData?.pairIndex ?? 0;
  const curveOffset =
    pairTotal > 1 ? (pairIndex - (pairTotal - 1) / 2) * 56 : 0;

  const { path, labelX, labelY } = computeFloatingPath(
    nodeRect(sourceNode),
    nodeRect(targetNode),
    curveOffset,
    edgeData?.canonicalForward ?? true,
  );

  return (
    <>
      <BaseEdge id={id} path={path} style={FLOATING_EDGE_STYLE} />
      {edgeData?.label && (
        <EdgeLabelRenderer>
          <div
            className="nodrag nopan pointer-events-none absolute rounded border border-primary/20 bg-base-100/95 px-1.5 py-0.5 text-[11px] leading-tight font-medium text-primary shadow-2xs"
            style={{
              transform: `translate(-50%, -50%) translate(${labelX}px, ${labelY}px)`,
            }}
          >
            {edgeData.label}
          </div>
        </EdgeLabelRenderer>
      )}
    </>
  );
});
