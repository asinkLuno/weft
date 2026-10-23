import dagre from "@dagrejs/dagre";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { Edge, Node } from "@xyflow/react";
import { type Lang, strings } from "@/i18n";
import type { WeftDoc } from "@/lib/document";
// 实体卡片节点：数据在 EntityView 里解析好，这里只管展示
export type EntityT = NonNullable<WeftDoc["entity"]>[string];
export type EntityData = {
  name: string;
  avatar?: string;
  baseTime?: string;
  parents: Array<{ id: string; name: string; preview?: EntityData }>;
  // extra_props 展开成排好序的键值对；值已转成展示文本
  extraProps: Array<[string, string]>;
  description?: string;
  belongLabel: string;
};
export type EntityFlowNode = Node<EntityData, "entity">;

// 从归属 DAG 收集直接归属；后端已保证 ID 合法且无环。
export function entityParents(
  dag: WeftDoc["belong_dag"],
): Map<string, Set<string>> {
  const parents = new Map<string, Set<string>>();
  for (const node of dag ?? []) {
    if (node.belong_to?.length) {
      parents.set(node.id, new Set(node.belong_to));
    }
  }
  return parents;
}

// 卡片实际高度随内容变化，dagre 按这个估值布点，特别高的卡片可能轻微重叠
export const ENTITY_W = 280;
export const ENTITY_H = 170;

// 没有用户拖放过位置的节点用 dagre 自动分层；返回各节点左上角坐标
export function dagreLayout(
  entries: Array<[string, EntityT]>,
  edges: Edge[],
): Record<string, { x: number; y: number }> {
  const g = new dagre.graphlib.Graph();
  g.setGraph({ rankdir: "TB", nodesep: 48, ranksep: 64 });
  g.setDefaultEdgeLabel(() => ({}));
  for (const [id] of entries) {
    g.setNode(id, { width: ENTITY_W, height: ENTITY_H });
  }
  for (const edge of edges) g.setEdge(edge.source, edge.target);
  dagre.layout(g);
  const out: Record<string, { x: number; y: number }> = {};
  for (const [id] of entries) {
    const { x, y } = g.node(id);
    out[id] = { x: x - ENTITY_W / 2, y: y - ENTITY_H / 2 };
  }
  return out;
}

export function buildEntityView(
  doc: WeftDoc | null,
  lang: Lang,
  dim: "belong" | "link" | null,
  pick: string | null,
  saved: Record<string, { x: number; y: number }>,
) {
  const t = strings[lang];
  const entities = Object.entries(doc?.entity ?? {}).sort(([a], [b]) =>
    a.localeCompare(b),
  );
  const nameOf = (id: string) => doc?.entity?.[id]?.id ?? id;

  // entity_link 的引用按 id 对回实体；未知引用取 null 丢弃（后端已校验，防御手滑）
  const idOf = (r: string) => (doc?.entity && r in doc.entity ? r : null);

  const parentsById = entityParents(doc?.belong_dag);

  // id 的全部上层归属（跨多级）；后端已保证无环，先占位只是防手滑自环
  const cache = new Map<string, Set<string>>();
  const ancestorsOf = (id: string): Set<string> => {
    const hit = cache.get(id);
    if (hit) return hit;
    const set = new Set<string>();
    cache.set(id, set);
    for (const p of parentsById.get(id) ?? []) {
      set.add(p);
      for (const a of ancestorsOf(p)) set.add(a);
    }
    return set;
  };

  // 出现过的所有归属目标；已不存在的选中项视为无效，不参与过滤
  const targets = [
    ...new Set([...parentsById.values()].flatMap((ids) => [...ids])),
  ];
  const activePick =
    dim === "belong" && pick && targets.includes(pick) ? pick : null;

  // 根 = 没有上级的实体。归属目标按根分组：多棵互不相连的归属树自动拆成
  // 多个 section，无需在文档里额外声明；组内按层级深度排（根在前），组间画分隔线
  const rootsOf = (id: string) => {
    const roots = [...ancestorsOf(id)].filter(
      (ancestor) => (parentsById.get(ancestor)?.size ?? 0) === 0,
    );
    return roots.length ? roots : [id];
  };
  const byRoot = new Map<string, Array<{ v: string; label: string }>>();
  for (const target of targets) {
    for (const root of rootsOf(target)) {
      const section = byRoot.get(root) ?? [];
      section.push({ v: target, label: nameOf(target) });
      byRoot.set(root, section);
    }
  }
  const belongSections = [...byRoot.entries()]
    .sort(([a], [b]) => nameOf(a).localeCompare(nameOf(b)))
    .map(([, section]) =>
      section.sort(
        (a, b) =>
          ancestorsOf(a.v).size - ancestorsOf(b.v).size ||
          a.label.localeCompare(b.label),
      ),
    );

  // 关系组筛选同理：组在文件更新后可能消失，失效视为未选
  const groups = Object.keys(doc?.entity_link ?? {}).sort();
  const activeLink =
    dim === "link" && pick && groups.includes(pick) ? pick : null;

  // 选中组内出现过的实体（解析成 id）
  const linkMembers = new Set(
    activeLink
      ? (doc?.entity_link?.[activeLink] ?? [])
          .flatMap(({ entities: [a, b] }) => [idOf(a), idOf(b)])
          .filter((x): x is string => x !== null)
      : [],
  );

  const filtered = entities.filter(([id]) => {
    // 选中的实体自己也算命中，层级图里当根展示
    if (activePick && id !== activePick && !ancestorsOf(id).has(activePick))
      return false;
    if (activeLink && !linkMembers.has(id)) return false;
    return true;
  });

  // extra_props 的展示：标量直接转字符串，复合值落成 JSON 文本
  function formatExtraValue(value: unknown): string {
    return typeof value === "object" && value !== null
      ? JSON.stringify(value)
      : String(value);
  }

  const entityData = (
    id: string,
    e: EntityT,
    includePreviews = true,
  ): EntityData => ({
    name: nameOf(id),
    avatar: e.avatar ? convertFileSrc(e.avatar) : undefined,
    baseTime:
      e.base_time_humanizer !== undefined
        ? `${t.baseTime}: ${e.base_time_humanizer}`
        : undefined,
    parents: [...(parentsById.get(id) ?? [])].map((b) => ({
      id: b,
      name: nameOf(b),
      preview:
        includePreviews && doc?.entity?.[b]
          ? entityData(b, doc.entity[b], false)
          : undefined,
    })),
    extraProps: Object.entries(e.extra_props ?? {})
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([key, value]) => [key, formatExtraValue(value)]),
    description: e.description,
    belongLabel: t.belongTo,
  });

  // 有归属筛选时按 DAG 的归属连边、dagre 分层；无筛选不连归属线、摆成网格平铺
  const visibleSet = new Set(filtered.map(([id]) => id));
  const edges: Edge[] = filtered.flatMap(([id]) =>
    [...(parentsById.get(id) ?? [])]
      .filter((p) => visibleSet.has(p))
      .map((p) => ({ id: `${p}->${id}`, source: p, target: id })),
  );

  // 关系对只在关系筛选视图展示；端点不在当前过滤集时整条隐藏
  const linkEdges: Edge[] = Object.entries(doc?.entity_link ?? {})
    .filter(([group]) => group === activeLink)
    .flatMap(([group, links]) =>
      links.flatMap(({ entities: [a, b], relations }, i) => {
        const s = idOf(a);
        const t = idOf(b);
        return s && t && s !== t && visibleSet.has(s) && visibleSet.has(t)
          ? [
              {
                id: `link:${group}:${i}`,
                source: s,
                target: t,
                label: relations,
                labelStyle: { fontSize: 12 },
              },
            ]
          : [];
      }),
    );
  const auto: Record<string, { x: number; y: number }> = {};
  if (activePick || activeLink) {
    // 归属筛选按归属连边分层，关系筛选按关系线分层；两个维度互斥
    Object.assign(auto, dagreLayout(filtered, activePick ? edges : linkEdges));
  } else {
    const cols = Math.ceil(Math.sqrt(filtered.length));
    filtered.forEach(([id], i) => {
      auto[id] = {
        x: (i % cols) * (ENTITY_W + 24),
        y: Math.floor(i / cols) * (ENTITY_H + 24),
      };
    });
  }
  // 筛选视图始终按 dagre 全量重排（不读、不存用户摆位）；平铺/搜索视图尊重拖过的位置
  const nodes: EntityFlowNode[] = filtered.map(([id, e]) => ({
    id,
    type: "entity",
    position: activePick || activeLink ? auto[id] : (saved[id] ?? auto[id]),
    data: entityData(id, e),
  }));

  return {
    nodes,
    edges: activePick ? edges : activeLink ? linkEdges : [],
    activePick,
    activeLink,
    belongSections,
    groups,
    filtered,
    nameOf,
  };
}
