import { convertFileSrc } from "@tauri-apps/api/core";
import { type Edge, MarkerType, type Node } from "@xyflow/react";
import { type Lang, strings } from "@/i18n";
import type { WeftDoc } from "@/lib/document";
import { formatTimeRange } from "@/lib/ui";
import {
  type EntityLayoutMode,
  gridLayout,
  type NodeSize,
  type Point,
} from "./layout";

export type EntityT = NonNullable<WeftDoc["entity"]>[string];

export type EntityFilter =
  | { dim: "belong"; pick: string }
  | { dim: "link"; pick: string }
  | { dim: "tags"; picks: readonly string[] };

export type TagOption = {
  tag: string;
  count: number;
};

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
  tags: string[];
  parents: EntityParentRef[];
  extraProps: Array<[string, string]>;
  description?: string;
  belongLabel: string;
};

export type EntityFlowNode = Node<EntityData, "entity">;

export type FloatingEdgeData = {
  label: string;
  directed: boolean;
  pairIndex: number;
  pairTotal: number;
  canonicalForward: boolean;
};

export type EntityLinkDirection = "undirected" | "outgoing" | "incoming";

export type EntityLinkEntry = {
  key: string;
  group: string;
  relation: string;
  direction: EntityLinkDirection;
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
  tags: string[];
  extraProps: Array<[string, string]>;
  parents: Array<{ id: string; label: string }>;
  children: Array<{ id: string; label: string }>;
  links: EntityLinkEntry[];
  events: EntityEventEntry[];
};

const EMPTY_SIZES: ReadonlyMap<string, NodeSize> = new Map();

type EntityDocIndex = {
  sortedEntities: ReadonlyArray<[string, EntityT]>;
  parentsById: Map<string, Set<string>>;
  ancestorsOf: (id: string) => Set<string>;
  belongTargetSet: ReadonlySet<string>;
  belongSections: Array<Array<{ v: string; label: string }>>;
  linkSections: Array<Array<{ v: string; label: string }>>;
  tagCounts: ReadonlyMap<string, number>;
  tagOptions: ReadonlyArray<TagOption>;
  childrenById: Map<string, Array<{ id: string; label: string }>>;
  linksByEntity: Map<string, EntityLinkEntry[]>;
  eventsByEntity: Map<string, EntityEventEntry[]>;
};

const entityIndexCache = new WeakMap<WeftDoc, EntityDocIndex>();

// 按 WeftDoc 实例缓存文档级派生索引：筛选切换零重复排序/DAG遍历/标签聚合，详情查表 O(1)
function getEntityDocIndex(doc: WeftDoc): EntityDocIndex {
  const cached = entityIndexCache.get(doc);
  if (cached) return cached;

  const entityMap = doc.entity ?? {};
  const sortedEntities = Object.entries(entityMap).sort(([a], [b]) =>
    a.localeCompare(b),
  );
  const nameOf = (id: string) => entityMap[id]?.id ?? id;
  const parentsById = entityParents(doc.belong_dag);
  const ancestorsOf = createAncestorsLookup(parentsById);
  const { targetSet: belongTargetSet, belongSections } = buildBelongSections(
    parentsById,
    ancestorsOf,
    nameOf,
  );

  const tagCounts = new Map<string, number>();
  for (const [, entity] of sortedEntities) {
    for (const tag of entity.tags ?? []) {
      tagCounts.set(tag, (tagCounts.get(tag) ?? 0) + 1);
    }
  }
  const tagOptions: TagOption[] = [...tagCounts.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([tag, count]) => ({ tag, count }));

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
  const linkSections = [sortedGroups.map(([g]) => ({ v: g, label: g }))];
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
        directed,
      } = entries[i];
      const entA = entityMap[a];
      const entB = entityMap[b];
      if (!entA || !entB) continue;
      pushLink(a, {
        key: `${group}:${i}:a`,
        group,
        relation: relations,
        direction: directed ? "outgoing" : "undirected",
        peerId: b,
        peerName: entB.id,
      });
      if (a !== b) {
        pushLink(b, {
          key: `${group}:${i}:b`,
          group,
          relation: relations,
          direction: directed ? "incoming" : "undirected",
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
    sortedEntities,
    parentsById,
    ancestorsOf,
    belongTargetSet,
    belongSections,
    linkSections,
    tagCounts,
    tagOptions,
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
  directed: boolean;
  pairKey: string;
  canonicalForward: boolean;
};

function collectValidLinks(
  activeLinks: ReadonlyArray<{
    entities: [string, string];
    relations: string;
    directed: boolean;
  }>,
  visibleSet: ReadonlySet<string>,
): LinkEdgeRecord[] {
  const valid: LinkEdgeRecord[] = [];
  for (let i = 0; i < activeLinks.length; i++) {
    const {
      entities: [source, target],
      relations,
      directed,
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
      directed,
      pairKey,
      canonicalForward,
    });
  }
  return valid;
}

const DIRECTED_MARKER = {
  type: MarkerType.ArrowClosed,
  color: "var(--color-primary)",
  width: 18,
  height: 18,
} as const;

function buildLinkEdges(
  activeLink: string,
  activeLinks: ReadonlyArray<{
    entities: [string, string];
    relations: string;
    directed: boolean;
  }>,
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
      directed: item.directed,
      pairIndex,
      pairTotal: pairCounts.get(item.pairKey) ?? 1,
      canonicalForward: item.canonicalForward,
    };
    return {
      id: `link:${activeLink}:${item.index}`,
      type: "floating",
      source: item.source,
      target: item.target,
      markerEnd: item.directed ? DIRECTED_MARKER : undefined,
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
    tags: entity.tags ?? [],
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
      tags: e.tags ?? [],
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
  activeTags: readonly string[],
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
  if (activeTags.length > 0) {
    return entities.filter(([, entity]) => {
      const entityTags = entity.tags;
      if (!entityTags || entityTags.length < activeTags.length) return false;
      return activeTags.every((tag) => entityTags.includes(tag));
    });
  }
  return [...entities];
}

const EMPTY_DOC_INDEX: EntityDocIndex = {
  sortedEntities: [],
  parentsById: new Map(),
  ancestorsOf: () => new Set(),
  belongTargetSet: new Set(),
  belongSections: [],
  linkSections: [[]],
  tagCounts: new Map(),
  tagOptions: [],
  childrenById: new Map(),
  linksByEntity: new Map(),
  eventsByEntity: new Map(),
};

export function buildEntityView(
  doc: WeftDoc | null,
  lang: Lang,
  filter: EntityFilter | null,
  saved: Readonly<Record<string, Point>>,
) {
  const t = strings[lang];
  const entityMap = doc?.entity ?? {};
  const index = doc ? getEntityDocIndex(doc) : EMPTY_DOC_INDEX;
  const {
    sortedEntities: entities,
    parentsById,
    ancestorsOf,
    belongTargetSet: targetSet,
    belongSections,
    linkSections,
    tagCounts,
    tagOptions,
  } = index;
  const nameOf = (id: string) => entityMap[id]?.id ?? id;

  const activePick =
    filter?.dim === "belong" && targetSet.has(filter.pick) ? filter.pick : null;
  const linkGroups = doc?.entity_link ?? {};
  const activeLink =
    filter?.dim === "link" && filter.pick in linkGroups ? filter.pick : null;
  const activeLinks = activeLink ? (linkGroups[activeLink] ?? []) : [];

  const activeTags =
    filter?.dim === "tags"
      ? [...new Set(filter.picks.filter((tag) => tagCounts.has(tag)))].sort(
          (a, b) => a.localeCompare(b),
        )
      : [];

  const filtered = filterEntities(
    entities,
    activePick,
    activeLink,
    activeTags,
    ancestorsOf,
    collectLinkMembers(activeLinks, entityMap),
  );

  const layoutMode: EntityLayoutMode = activePick
    ? "hierarchy"
    : activeLink
      ? "network"
      : "grid";
  const useSavedPositions = layoutMode === "grid" && activeTags.length === 0;

  const visibleIds = filtered.map(([id]) => id);
  const visibleSet = new Set(visibleIds);
  const edges = activePick
    ? buildBelongEdges(filtered, parentsById, visibleSet)
    : activeLink
      ? buildLinkEdges(activeLink, activeLinks, visibleSet)
      : [];

  const entityData = createEntityDataBuilder(entityMap, parentsById, t);
  // 首次测量前的占位坐标用 O(n) 网格布局，避免同步加载 dagre；测量完成后由 EntityCanvas 精准重排
  const auto = gridLayout(visibleIds, EMPTY_SIZES);
  const nodes: EntityFlowNode[] = filtered.map(([id, e]) => ({
    id,
    type: "entity",
    position: useSavedPositions ? (saved[id] ?? auto[id]) : auto[id],
    data: entityData(id, e),
  }));

  return {
    nodes,
    edges,
    layoutMode,
    useSavedPositions,
    activePick,
    activeLink,
    activeTags,
    belongSections,
    linkSections,
    tagOptions,
    nameOf,
  };
}
