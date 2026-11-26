import { convertFileSrc } from "@tauri-apps/api/core";
import type { Edge, Node } from "@xyflow/react";
import { type Lang, strings } from "@/i18n";
import type { WeftDoc } from "@/lib/document";
import { formatTimeRange } from "@/lib/ui";
import {
  computeEntityLayout,
  type EntityLayoutMode,
  type NodeSize,
  type Point,
} from "./layout";

export type EntityT = NonNullable<WeftDoc["entity"]>[string];

export type EntityParentRef = {
  id: string;
  name: string;
  preview?: EntityData;
};

export type EntityData = {
  id: string;
  name: string;
  avatar?: string;
  baseTime?: string;
  parents: EntityParentRef[];
  extraProps: Array<[string, string]>;
  description?: string;
  belongLabel: string;
};

export type EntityFlowNode = Node<EntityData, "entity">;

export type FloatingEdgeData = {
  label: string;
  pairIndex: number;
  pairTotal: number;
  canonicalForward: boolean;
};

export type EntityLinkEntry = {
  key: string;
  group: string;
  relation: string;
  peerId: string;
  peerName: string;
};

export type EntityEventEntry = {
  id: string;
  time?: string;
  description?: string;
};

export type EntityDetail = {
  id: string;
  name: string;
  avatar?: string;
  baseTime?: string;
  description?: string;
  extraProps: Array<[string, string]>;
  parents: Array<{ id: string; label: string }>;
  children: Array<{ id: string; label: string }>;
  links: EntityLinkEntry[];
  events: EntityEventEntry[];
};

const EMPTY_SIZES: ReadonlyMap<string, NodeSize> = new Map();
const BELONG_EDGE_STYLE = {
  stroke: "var(--color-base-content)",
  strokeOpacity: 0.35,
  strokeWidth: 1.5,
} as const;

type EntityDocIndex = {
  parentsById: Map<string, Set<string>>;
  childrenById: Map<string, Array<{ id: string; label: string }>>;
  linksByEntity: Map<string, EntityLinkEntry[]>;
  eventsByEntity: Map<string, EntityEventEntry[]>;
};

const entityIndexCache = new WeakMap<WeftDoc, EntityDocIndex>();

// 按 WeftDoc 实例缓存反向索引：点击不同实体查详情 O(1)，同时与 buildEntityView 复用 parentsById
function getEntityDocIndex(doc: WeftDoc): EntityDocIndex {
  const cached = entityIndexCache.get(doc);
  if (cached) return cached;

  const entityMap = doc.entity ?? {};
  const parentsById = entityParents(doc.belong_dag);

  const childrenById = new Map<string, Array<{ id: string; label: string }>>();
  for (const [childId, pSet] of parentsById.entries()) {
    const child = entityMap[childId];
    if (!child) continue;
    for (const parentId of pSet) {
      let list = childrenById.get(parentId);
      if (!list) {
        list = [];
        childrenById.set(parentId, list);
      }
      list.push({ id: childId, label: child.id });
    }
  }
  for (const list of childrenById.values()) {
    list.sort((a, b) => a.label.localeCompare(b.label));
  }

  const linksByEntity = new Map<string, EntityLinkEntry[]>();
  const sortedGroups = Object.entries(doc.entity_link ?? {}).sort(([a], [b]) =>
    a.localeCompare(b),
  );
  const pushLink = (entityId: string, entry: EntityLinkEntry) => {
    let list = linksByEntity.get(entityId);
    if (!list) {
      list = [];
      linksByEntity.set(entityId, list);
    }
    list.push(entry);
  };
  for (const [group, entries] of sortedGroups) {
    for (let i = 0; i < entries.length; i++) {
      const {
        entities: [a, b],
        relations,
      } = entries[i];
      const entA = entityMap[a];
      const entB = entityMap[b];
      if (!entA || !entB) continue;
      pushLink(a, {
        key: `${group}:${i}:a`,
        group,
        relation: relations,
        peerId: b,
        peerName: entB.id,
      });
      if (a !== b) {
        pushLink(b, {
          key: `${group}:${i}:b`,
          group,
          relation: relations,
          peerId: a,
          peerName: entA.id,
        });
      }
    }
  }

  const rawEventsByEntity = new Map<
    string,
    Array<EntityEventEntry & { tick: number }>
  >();
  for (const [id, ev] of Object.entries(doc.event ?? {})) {
    const participants = ev.entities;
    if (!participants?.length) continue;
    const entry: EntityEventEntry & { tick: number } = {
      id,
      time: formatTimeRange(ev.start_display, ev.end_display),
      description: ev.description,
      tick: ev.start_tick ?? Number.MAX_SAFE_INTEGER,
    };
    const seen = participants.length > 1 ? new Set<string>() : null;
    for (const entityId of participants) {
      if (seen) {
        if (seen.has(entityId)) continue;
        seen.add(entityId);
      }
      let list = rawEventsByEntity.get(entityId);
      if (!list) {
        list = [];
        rawEventsByEntity.set(entityId, list);
      }
      list.push(entry);
    }
  }
  const eventsByEntity = new Map<string, EntityEventEntry[]>();
  for (const [entityId, list] of rawEventsByEntity.entries()) {
    list.sort((a, b) => a.tick - b.tick || a.id.localeCompare(b.id));
    eventsByEntity.set(
      entityId,
      list.map(({ id, time, description }) => ({ id, time, description })),
    );
  }

  const index: EntityDocIndex = {
    parentsById,
    childrenById,
    linksByEntity,
    eventsByEntity,
  };
  entityIndexCache.set(doc, index);
  return index;
}

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

function formatExtraProps(
  extraProps?: Record<string, unknown>,
): Array<[string, string]> {
  return Object.entries(extraProps ?? {})
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([key, value]) => [
      key,
      typeof value === "object" && value !== null
        ? JSON.stringify(value)
        : String(value),
    ]);
}

function createAncestorsLookup(
  parentsById: ReadonlyMap<string, Set<string>>,
): (id: string) => Set<string> {
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
  return ancestorsOf;
}

function buildBelongSections(
  parentsById: ReadonlyMap<string, Set<string>>,
  ancestorsOf: (id: string) => Set<string>,
  nameOf: (id: string) => string,
) {
  const targetSet = new Set<string>();
  for (const ids of parentsById.values()) {
    for (const id of ids) targetSet.add(id);
  }

  const rootsOf = (id: string) => {
    const roots = [...ancestorsOf(id)].filter(
      (ancestor) => (parentsById.get(ancestor)?.size ?? 0) === 0,
    );
    return roots.length ? roots : [id];
  };

  const byRoot = new Map<string, Array<{ v: string; label: string }>>();
  for (const target of targetSet) {
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

  return { targetSet, belongSections };
}

function buildBelongEdges(
  filtered: ReadonlyArray<[string, EntityT]>,
  parentsById: ReadonlyMap<string, Set<string>>,
  visibleSet: ReadonlySet<string>,
): Edge[] {
  const edges: Edge[] = [];
  for (const [id] of filtered) {
    const parents = parentsById.get(id);
    if (!parents) continue;
    for (const p of parents) {
      if (!visibleSet.has(p)) continue;
      edges.push({
        id: `${p}->${id}`,
        source: p,
        target: id,
        style: BELONG_EDGE_STYLE,
      });
    }
  }
  return edges;
}

type LinkEdgeRecord = {
  index: number;
  source: string;
  target: string;
  relation: string;
  pairKey: string;
  canonicalForward: boolean;
};

function collectValidLinks(
  activeLinks: ReadonlyArray<{ entities: [string, string]; relations: string }>,
  visibleSet: ReadonlySet<string>,
): LinkEdgeRecord[] {
  const valid: LinkEdgeRecord[] = [];
  for (let i = 0; i < activeLinks.length; i++) {
    const {
      entities: [source, target],
      relations,
    } = activeLinks[i];
    if (source === target || !visibleSet.has(source) || !visibleSet.has(target))
      continue;
    const canonicalForward = source < target;
    const pairKey = canonicalForward
      ? `${source}\0${target}`
      : `${target}\0${source}`;
    valid.push({
      index: i,
      source,
      target,
      relation: relations,
      pairKey,
      canonicalForward,
    });
  }
  return valid;
}

function buildLinkEdges(
  activeLink: string,
  activeLinks: ReadonlyArray<{ entities: [string, string]; relations: string }>,
  visibleSet: ReadonlySet<string>,
): Edge[] {
  const valid = collectValidLinks(activeLinks, visibleSet);
  const pairCounts = new Map<string, number>();
  for (const item of valid) {
    pairCounts.set(item.pairKey, (pairCounts.get(item.pairKey) ?? 0) + 1);
  }

  const pairSeen = new Map<string, number>();
  return valid.map((item) => {
    const pairIndex = pairSeen.get(item.pairKey) ?? 0;
    pairSeen.set(item.pairKey, pairIndex + 1);
    const data: FloatingEdgeData = {
      label: item.relation,
      pairIndex,
      pairTotal: pairCounts.get(item.pairKey) ?? 1,
      canonicalForward: item.canonicalForward,
    };
    return {
      id: `link:${activeLink}:${item.index}`,
      type: "floating",
      source: item.source,
      target: item.target,
      data,
    };
  });
}

export function buildEntityDetail(
  doc: WeftDoc | null,
  lang: Lang,
  entityId: string,
): EntityDetail | null {
  const entityMap = doc?.entity;
  const entity = entityMap?.[entityId];
  if (!doc || !entity || !entityMap) return null;

  const t = strings[lang];
  const { parentsById, childrenById, linksByEntity, eventsByEntity } =
    getEntityDocIndex(doc);
  const parents = [...(parentsById.get(entityId) ?? [])]
    .map((id) => ({ id, label: entityMap[id]?.id ?? id }))
    .sort((a, b) => a.label.localeCompare(b.label));

  return {
    id: entityId,
    name: entity.id,
    avatar: entity.avatar ? convertFileSrc(entity.avatar) : undefined,
    baseTime:
      entity.base_time_humanizer !== undefined
        ? `${t.baseTime}: ${entity.base_time_humanizer}`
        : undefined,
    description: entity.description,
    extraProps: formatExtraProps(entity.extra_props),
    parents,
    children: childrenById.get(entityId) ?? [],
    links: linksByEntity.get(entityId) ?? [],
    events: eventsByEntity.get(entityId) ?? [],
  };
}

function createEntityDataBuilder(
  entityMap: Readonly<Record<string, EntityT>>,
  parentsById: ReadonlyMap<string, Set<string>>,
  t: (typeof strings)[Lang],
) {
  const previewCache = new Map<string, EntityData>();
  const build = (
    id: string,
    e: EntityT,
    includePreviews = true,
  ): EntityData => {
    if (!includePreviews) {
      const cached = previewCache.get(id);
      if (cached) return cached;
    }
    const built: EntityData = {
      id,
      name: e.id,
      avatar: e.avatar ? convertFileSrc(e.avatar) : undefined,
      baseTime:
        e.base_time_humanizer !== undefined
          ? `${t.baseTime}: ${e.base_time_humanizer}`
          : undefined,
      parents: [...(parentsById.get(id) ?? [])].map((b) => ({
        id: b,
        name: entityMap[b]?.id ?? b,
        preview:
          includePreviews && entityMap[b]
            ? build(b, entityMap[b], false)
            : undefined,
      })),
      extraProps: formatExtraProps(e.extra_props),
      description: e.description,
      belongLabel: t.belongTo,
    };
    if (!includePreviews) previewCache.set(id, built);
    return built;
  };
  return build;
}

function collectLinkMembers(
  activeLinks: ReadonlyArray<{ entities: [string, string] }>,
  entityMap: Readonly<Record<string, EntityT>>,
): Set<string> {
  const members = new Set<string>();
  for (const {
    entities: [a, b],
  } of activeLinks) {
    if (a in entityMap) members.add(a);
    if (b in entityMap) members.add(b);
  }
  return members;
}

function filterEntities(
  entities: ReadonlyArray<[string, EntityT]>,
  activePick: string | null,
  activeLink: string | null,
  ancestorsOf: (id: string) => Set<string>,
  linkMembers: ReadonlySet<string>,
): Array<[string, EntityT]> {
  if (activePick) {
    return entities.filter(
      ([id]) => id === activePick || ancestorsOf(id).has(activePick),
    );
  }
  if (activeLink) {
    return entities.filter(([id]) => linkMembers.has(id));
  }
  return [...entities];
}

export function buildEntityView(
  doc: WeftDoc | null,
  lang: Lang,
  dim: "belong" | "link" | null,
  pick: string | null,
  saved: Readonly<Record<string, Point>>,
) {
  const t = strings[lang];
  const entityMap = doc?.entity ?? {};
  const entities = Object.entries(entityMap).sort(([a], [b]) =>
    a.localeCompare(b),
  );
  const nameOf = (id: string) => entityMap[id]?.id ?? id;
  const parentsById = doc
    ? getEntityDocIndex(doc).parentsById
    : entityParents(undefined);
  const ancestorsOf = createAncestorsLookup(parentsById);
  const { targetSet, belongSections } = buildBelongSections(
    parentsById,
    ancestorsOf,
    nameOf,
  );

  const activePick =
    dim === "belong" && pick && targetSet.has(pick) ? pick : null;
  const linkGroups = doc?.entity_link ?? {};
  const groups = Object.keys(linkGroups).sort();
  const linkSections = [groups.map((g) => ({ v: g, label: g }))];
  const activeLink = dim === "link" && pick && pick in linkGroups ? pick : null;
  const activeLinks = activeLink ? (linkGroups[activeLink] ?? []) : [];

  const filtered = filterEntities(
    entities,
    activePick,
    activeLink,
    ancestorsOf,
    collectLinkMembers(activeLinks, entityMap),
  );

  const layoutMode: EntityLayoutMode = activePick
    ? "hierarchy"
    : activeLink
      ? "network"
      : "grid";

  const visibleIds = filtered.map(([id]) => id);
  const visibleSet = new Set(visibleIds);
  const edges = activePick
    ? buildBelongEdges(filtered, parentsById, visibleSet)
    : activeLink
      ? buildLinkEdges(activeLink, activeLinks, visibleSet)
      : [];

  const entityData = createEntityDataBuilder(entityMap, parentsById, t);
  const auto = computeEntityLayout(layoutMode, visibleIds, edges, EMPTY_SIZES);
  const nodes: EntityFlowNode[] = filtered.map(([id, e]) => ({
    id,
    type: "entity",
    position: layoutMode === "grid" ? (saved[id] ?? auto[id]) : auto[id],
    data: entityData(id, e),
  }));

  return {
    nodes,
    edges,
    layoutMode,
    activePick,
    activeLink,
    belongSections,
    linkSections,
    nameOf,
  };
}
