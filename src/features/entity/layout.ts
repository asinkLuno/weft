export type NodeSize = { width: number; height: number };
export type Point = { x: number; y: number };
export type LayoutEdge = { source: string; target: string };
export type EntityLayoutMode = "grid" | "hierarchy" | "network";

export const DEFAULT_NODE_SIZE: NodeSize = { width: 280, height: 180 };

type DagreModule = typeof import("@dagrejs/dagre")["default"];
let dagrePromise: Promise<DagreModule> | null = null;

function loadDagre(): Promise<DagreModule> {
  if (!dagrePromise) {
    dagrePromise = import("@dagrejs/dagre").then((m) => m.default);
  }
  return dagrePromise;
}

function sizeOf(sizes: ReadonlyMap<string, NodeSize>, id: string): NodeSize {
  return sizes.get(id) ?? DEFAULT_NODE_SIZE;
}

/**
 * 动态行高网格布局：按每行内最高的真实卡片高度累加行距，避免高卡片上下重叠。
 */
export function gridLayout(
  ids: readonly string[],
  sizes: ReadonlyMap<string, NodeSize>,
  gap = 24,
): Record<string, Point> {
  const out: Record<string, Point> = {};
  if (ids.length === 0) return out;

  const cols = Math.max(1, Math.ceil(Math.sqrt(ids.length)));
  const colWidths = new Array<number>(cols).fill(0);
  for (let i = 0; i < ids.length; i++) {
    const col = i % cols;
    colWidths[col] = Math.max(colWidths[col], sizeOf(sizes, ids[i]).width);
  }

  const colX = new Array<number>(cols).fill(0);
  for (let col = 1; col < cols; col++) {
    colX[col] = colX[col - 1] + colWidths[col - 1] + gap;
  }

  let y = 0;
  for (let rowStart = 0; rowStart < ids.length; rowStart += cols) {
    let rowHeight = 0;
    const rowEnd = Math.min(ids.length, rowStart + cols);
    for (let i = rowStart; i < rowEnd; i++) {
      const id = ids[i];
      out[id] = { x: colX[i - rowStart], y };
      rowHeight = Math.max(rowHeight, sizeOf(sizes, id).height);
    }
    y += rowHeight + gap;
  }

  return out;
}

/**
 * 层级 DAG 布局（无环关系视图）：仅在 hierarchy 模式下按需加载 dagre，按真实宽高分层。
 */
export async function dagreLayout(
  ids: readonly string[],
  edges: readonly LayoutEdge[],
  sizes: ReadonlyMap<string, NodeSize>,
): Promise<Record<string, Point>> {
  const dagre = await loadDagre();
  const g = new dagre.graphlib.Graph();
  g.setGraph({ rankdir: "BT", nodesep: 48, ranksep: 64 });
  g.setDefaultEdgeLabel(() => ({}));
  for (const id of ids) {
    const { width, height } = sizeOf(sizes, id);
    g.setNode(id, { width, height });
  }
  for (const edge of edges) {
    g.setEdge(edge.source, edge.target);
  }
  dagre.layout(g);

  const out: Record<string, Point> = {};
  for (const id of ids) {
    const { width, height } = sizeOf(sizes, id);
    const pos = g.node(id);
    out[id] = { x: pos.x - width / 2, y: pos.y - height / 2 };
  }
  return out;
}

function buildAdjacency(
  ids: readonly string[],
  edges: readonly LayoutEdge[],
): Map<string, string[]> {
  const adj = new Map<string, string[]>(ids.map((id) => [id, []]));
  for (const { source, target } of edges) {
    if (source === target) continue;
    const fromList = adj.get(source);
    const toList = adj.get(target);
    if (!fromList || !toList) continue;
    fromList.push(target);
    toList.push(source);
  }
  return adj;
}

function collectComponent(
  start: string,
  adj: ReadonlyMap<string, string[]>,
  visited: Set<string>,
): string[] {
  const comp: string[] = [];
  const queue = [start];
  visited.add(start);
  for (let head = 0; head < queue.length; head++) {
    const cur = queue[head];
    comp.push(cur);
    for (const next of adj.get(cur) ?? []) {
      if (visited.has(next)) continue;
      visited.add(next);
      queue.push(next);
    }
  }
  return comp;
}

function splitComponents(
  ids: readonly string[],
  edges: readonly LayoutEdge[],
): string[][] {
  const adj = buildAdjacency(ids, edges);
  const visited = new Set<string>();
  const components: string[][] = [];
  for (const id of ids) {
    if (!visited.has(id)) {
      components.push(collectComponent(id, adj, visited));
    }
  }
  return components.sort((a, b) => b.length - a.length);
}

type BoxNode = {
  id: string;
  cx: number;
  cy: number;
  w: number;
  h: number;
};

type ComponentLayout = {
  positions: Record<string, Point>;
  width: number;
  height: number;
};

function separateBoxes(nodes: BoxNode[], padX: number, padY: number): boolean {
  let moved = false;
  for (let i = 0; i < nodes.length; i++) {
    const a = nodes[i];
    for (let j = i + 1; j < nodes.length; j++) {
      const b = nodes[j];
      const dx = b.cx - a.cx;
      const dy = b.cy - a.cy;
      const overlapX = (a.w + b.w) / 2 + padX - Math.abs(dx);
      const overlapY = (a.h + b.h) / 2 + padY - Math.abs(dy);
      if (overlapX <= 0 || overlapY <= 0) continue;

      moved = true;
      if (overlapX < overlapY) {
        const push = overlapX / 2 + 0.5;
        const sign = dx === 0 ? (i % 2 === 0 ? 1 : -1) : Math.sign(dx);
        a.cx -= sign * push;
        b.cx += sign * push;
      } else {
        const push = overlapY / 2 + 0.5;
        const sign = dy === 0 ? (j % 2 === 0 ? 1 : -1) : Math.sign(dy);
        a.cy -= sign * push;
        b.cy += sign * push;
      }
    }
  }
  return moved;
}

function uniqueUndirectedLinks(
  indexById: ReadonlyMap<string, number>,
  edges: readonly LayoutEdge[],
): Array<[number, number]> {
  const seen = new Set<string>();
  const links: Array<[number, number]> = [];
  for (const { source, target } of edges) {
    const a = indexById.get(source);
    const b = indexById.get(target);
    if (a === undefined || b === undefined || a === b) continue;
    const key = a < b ? `${a}:${b}` : `${b}:${a}`;
    if (seen.has(key)) continue;
    seen.add(key);
    links.push([a, b]);
  }
  return links;
}

function stepForceLayout(
  nodes: BoxNode[],
  links: readonly [number, number][],
  diags: Float64Array,
  fx: Float64Array,
  fy: Float64Array,
  cool: number,
): void {
  const n = nodes.length;
  fx.fill(0);
  fy.fill(0);

  for (let i = 0; i < n; i++) {
    const ni = nodes[i];
    const ix = ni.cx;
    const iy = ni.cy;
    const iw = ni.w;
    const ih = ni.h;
    for (let j = i + 1; j < n; j++) {
      const nj = nodes[j];
      const dx = nj.cx - ix;
      const dy = nj.cy - iy;
      const distSq = dx * dx + dy * dy;
      const dist = distSq > 0 ? Math.sqrt(distSq) : 1;
      const sumW = iw + nj.w;
      const sumH = ih + nj.h;
      const idealMin = Math.sqrt(sumW * sumW + sumH * sumH) * 0.45 + 90;
      if (dist < idealMin * 1.8) {
        const repulse = ((idealMin * idealMin) / (dist * dist)) * 18;
        const factor = repulse / dist;
        const rx = dx * factor;
        const ry = dy * factor;
        fx[i] -= rx;
        fy[i] -= ry;
        fx[j] += rx;
        fy[j] += ry;
      }
    }
  }

  for (const [a, b] of links) {
    const dx = nodes[b].cx - nodes[a].cx;
    const dy = nodes[b].cy - nodes[a].cy;
    const distSq = dx * dx + dy * dy;
    const dist = distSq > 0 ? Math.sqrt(distSq) : 1;
    const ideal = (diags[a] + diags[b]) * 0.38 + 110;
    const pull = (dist - ideal) * 0.08;
    const factor = pull / dist;
    const px = dx * factor;
    const py = dy * factor;
    fx[a] += px;
    fy[a] += py;
    fx[b] -= px;
    fy[b] -= py;
  }

  const maxStep = 36 * (0.2 + 0.8 * cool);
  for (let i = 0; i < n; i++) {
    const node = nodes[i];
    const forceX = fx[i] - node.cx * 0.012;
    const forceY = fy[i] - node.cy * 0.012;
    const magSq = forceX * forceX + forceY * forceY;
    if (magSq > 0) {
      const mag = Math.sqrt(magSq);
      const scale = Math.min(mag, maxStep) / mag;
      node.cx += forceX * scale;
      node.cy += forceY * scale;
    }
  }
}

function normalizeBoxLayout(nodes: readonly BoxNode[]): ComponentLayout {
  let minX = Number.POSITIVE_INFINITY;
  let minY = Number.POSITIVE_INFINITY;
  let maxX = Number.NEGATIVE_INFINITY;
  let maxY = Number.NEGATIVE_INFINITY;
  for (const node of nodes) {
    minX = Math.min(minX, node.cx - node.w / 2);
    minY = Math.min(minY, node.cy - node.h / 2);
    maxX = Math.max(maxX, node.cx + node.w / 2);
    maxY = Math.max(maxY, node.cy + node.h / 2);
  }

  const positions: Record<string, Point> = {};
  for (const node of nodes) {
    positions[node.id] = {
      x: Math.round(node.cx - node.w / 2 - minX),
      y: Math.round(node.cy - node.h / 2 - minY),
    };
  }
  return { positions, width: maxX - minX, height: maxY - minY };
}

function layoutComponent(
  compIds: readonly string[],
  edges: readonly LayoutEdge[],
  sizes: ReadonlyMap<string, NodeSize>,
): ComponentLayout {
  const n = compIds.length;
  const padX = 96;
  const padY = 72;

  if (n === 1) {
    const id = compIds[0];
    const s = sizeOf(sizes, id);
    return {
      positions: { [id]: { x: 0, y: 0 } },
      width: s.width,
      height: s.height,
    };
  }

  if (n === 2) {
    const [aId, bId] = compIds;
    const sa = sizeOf(sizes, aId);
    const sb = sizeOf(sizes, bId);
    const gapX = 140;
    const maxH = Math.max(sa.height, sb.height);
    return {
      positions: {
        [aId]: { x: 0, y: (maxH - sa.height) / 2 },
        [bId]: { x: sa.width + gapX, y: (maxH - sb.height) / 2 },
      },
      width: sa.width + gapX + sb.width,
      height: maxH,
    };
  }

  // 确定性圆周初始化，避免随机抖动与假的上下级层次
  const indexById = new Map(compIds.map((id, idx) => [id, idx]));
  const totalSpan = compIds.reduce((acc, id) => {
    const s = sizeOf(sizes, id);
    return acc + Math.max(s.width, s.height) + 80;
  }, 0);
  const radius = Math.max(220, totalSpan / (2 * Math.PI));

  const nodes: BoxNode[] = compIds.map((id, i) => {
    const s = sizeOf(sizes, id);
    const angle = -Math.PI / 2 + (2 * Math.PI * i) / n;
    return {
      id,
      cx: Math.cos(angle) * radius,
      cy: Math.sin(angle) * radius,
      w: s.width,
      h: s.height,
    };
  });

  const links = uniqueUndirectedLinks(indexById, edges);
  const diags = new Float64Array(n);
  for (let i = 0; i < n; i++) {
    diags[i] = Math.hypot(nodes[i].w, nodes[i].h);
  }
  const fx = new Float64Array(n);
  const fy = new Float64Array(n);
  const iterations = 140;
  for (let step = 0; step < iterations; step++) {
    stepForceLayout(nodes, links, diags, fx, fy, 1 - step / iterations);
    separateBoxes(nodes, padX, padY);
  }
  for (let pass = 0; pass < 32; pass++) {
    if (!separateBoxes(nodes, padX, padY)) break;
  }

  return normalizeBoxLayout(nodes);
}

/**
 * 网状关系布局（link 视图）：按连通分量排布，对等无上下级层次，按真实宽高防重叠。
 */
export function networkLayout(
  ids: readonly string[],
  edges: readonly LayoutEdge[],
  sizes: ReadonlyMap<string, NodeSize>,
): Record<string, Point> {
  const out: Record<string, Point> = {};
  if (ids.length === 0) return out;

  const components = splitComponents(ids, edges);
  const packed = components.map((comp) => layoutComponent(comp, edges, sizes));
  const compGap = 96;
  const totalArea = packed.reduce((sum, c) => sum + c.width * c.height, 0);
  const maxRowWidth = Math.max(1100, Math.ceil(Math.sqrt(totalArea)) * 1.35);

  let cursorX = 0;
  let cursorY = 0;
  let rowHeight = 0;
  for (const comp of packed) {
    if (cursorX > 0 && cursorX + comp.width > maxRowWidth) {
      cursorX = 0;
      cursorY += rowHeight + compGap;
      rowHeight = 0;
    }
    for (const [id, pos] of Object.entries(comp.positions)) {
      out[id] = { x: pos.x + cursorX, y: pos.y + cursorY };
    }
    cursorX += comp.width + compGap;
    rowHeight = Math.max(rowHeight, comp.height);
  }
  return out;
}

export async function computeEntityLayout(
  mode: EntityLayoutMode,
  ids: readonly string[],
  edges: readonly LayoutEdge[],
  sizes: ReadonlyMap<string, NodeSize>,
): Promise<Record<string, Point>> {
  switch (mode) {
    case "hierarchy":
      return dagreLayout(ids, edges, sizes);
    case "network":
      return networkLayout(ids, edges, sizes);
    case "grid":
      return gridLayout(ids, sizes);
  }
}
