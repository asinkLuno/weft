import { convertFileSrc } from "@tauri-apps/api/core";
import { type Edge, MarkerType, type Node } from "@xyflow/react";
import { type Lang, strings } from "@/i18n";
import type { WeftDoc, WeftLink } from "@/lib/document";
import { formatTimeRange } from "@/lib/ui";
import {
  type EntityLayoutMode,
  gridLayout,
  type NodeSize,
  type Point,
} from "./layout";

export type EntityT = NonNullable<WeftDoc["entity"]>[string];

export type EntityViewMode = "narrative" | "quotient";

export type EntityData = {
  id: string;
  name: string;
  aliases: string[];
  avatar?: string;
  baseTime?: string;
  extraProps: Array<[string, string]>;
  description?: string;
};

export type EntityFlowNode = Node<EntityData, "entity">;

export type FloatingEdgeData = {
  label?: string;
  directed: boolean;
  timeRange?: string;
  description?: string;
  pairIndex: number;
  pairTotal: number;
  canonicalForward: boolean;
};

export type EntityLinkDirection = "symmetric" | "outgoing" | "incoming";

export type EntityLinkEntry = {
  key: string;
  graph: string;
  label?: string;
  direction: EntityLinkDirection;
  peerId: string;
  peerName: string;
  startTick?: number;
  endTick?: number;
  timeRange?: string;
  description?: string;
};

export type EntityEventEntry = {
  id: string;
  time?: string;
  description?: string;
};

export type EntityDetail = {
  id: string;
  name: string;
  aliases: string[];
  avatar?: string;
  baseTime?: string;
  description?: string;
  extraProps: Array<[string, string]>;
  links: EntityLinkEntry[];
  events: EntityEventEntry[];
};

const EMPTY_SIZES: ReadonlyMap<string, NodeSize> = new Map();

const DIRECTED_MARKER = {
  type: MarkerType.ArrowClosed,
  color: "var(--color-primary)",
  width: 18,
  height: 18,
} as const;

type EntityDocIndex = {
  sortedEntities: ReadonlyArray<[string, EntityT]>;
  linkSections: Array<Array<{ v: string; label: string }>>;
  timeSections: Array<Array<{ v: string; label: string }>>;
  eventSliceTicks: ReadonlyMap<string, number>;
  linksByEntity: ReadonlyMap<string, EntityLinkEntry[]>;
  eventsByEntity: ReadonlyMap<string, EntityEventEntry[]>;
  hasEquivalence: boolean;
  repByEntity: ReadonlyMap<string, string>;
  membersByRep: ReadonlyMap<string, string[]>;
};

const entityIndexCache = new WeakMap<WeftDoc, EntityDocIndex>();

function formatLinkInterval(link: WeftLink): string | undefined {
  if (link.start && link.end) {
    return `${link.start.formatted} → ${link.end.formatted}`;
  }
  if (link.start) {
    return `${link.start.formatted} → +∞`;
  }
  if (link.end) {
    return `-∞ → ${link.end.formatted}`;
  }
  return undefined;
}

/**
 * 判断一条时变边在给定刻度 `tick` 是否处于生效状态（严格对齐后端 `AtomicSlice::Point(tick)`）：
 * - 默认省略 `start`/`end` 视为 `(-∞, +∞)`；
 * - 常规区间为左闭右开 `[start, end)`；
 * - 瞬时边 `start == end` 在且仅在 `tick == start` 时生效。
 */
export function isSpanActiveAtTick(
  startTick: number | undefined,
  endTick: number | undefined,
  tick: number,
): boolean {
  if (startTick !== undefined && startTick > tick) return false;
  if (endTick !== undefined) {
    if (startTick !== undefined && startTick === endTick) {
      return startTick === tick;
    }
    if (tick >= endTick) return false;
  }
  return true;
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

function computeEquivalenceClasses(
  entityMap: Readonly<Record<string, EntityT>>,
  links: readonly WeftLink[],
  graphs: Readonly<Record<string, NonNullable<WeftDoc["graphs"]>[string]>>,
): {
  hasEquivalence: boolean;
  repByEntity: Map<string, string>;
  membersByRep: Map<string, string[]>;
} {
  const parent = new Map<string, string>();
  const find = (x: string): string => {
    let curr = x;
    while (parent.get(curr) !== undefined && parent.get(curr) !== curr) {
      curr = parent.get(curr) as string;
    }
    let walk = x;
    while (walk !== curr) {
      const next = parent.get(walk) ?? curr;
      parent.set(walk, curr);
      walk = next;
    }
    return curr;
  };
  const union = (a: string, b: string) => {
    const ra = find(a);
    const rb = find(b);
    if (ra !== rb) parent.set(rb, ra);
  };

  for (const id of Object.keys(entityMap)) {
    parent.set(id, id);
  }
  for (const link of links) {
    const g = graphs[link.graph];
    if (!g?.equivalence || g.domain === "event") continue;
    if (!(link.from in entityMap) || !(link.to in entityMap)) continue;
    union(link.from, link.to);
  }

  const buckets = new Map<string, string[]>();
  for (const id of Object.keys(entityMap)) {
    const root = find(id);
    const list = buckets.get(root) ?? [];
    list.push(id);
    buckets.set(root, list);
  }

  const repByEntity = new Map<string, string>();
  const membersByRep = new Map<string, string[]>();
  let hasEquivalence = false;

  for (const members of buckets.values()) {
    members.sort((a, b) => {
      const scoreA =
        (entityMap[a]?.avatar ? 2 : 0) + (entityMap[a]?.description ? 1 : 0);
      const scoreB =
        (entityMap[b]?.avatar ? 2 : 0) + (entityMap[b]?.description ? 1 : 0);
      if (scoreA !== scoreB) return scoreB - scoreA;
      return a.localeCompare(b);
    });
    const rep = members[0];
    if (members.length > 1) hasEquivalence = true;
    membersByRep.set(rep, members);
    for (const m of members) {
      repByEntity.set(m, rep);
    }
  }

  return { hasEquivalence, repByEntity, membersByRep };
}

function getEntityDocIndex(doc: WeftDoc): EntityDocIndex {
  const cached = entityIndexCache.get(doc);
  if (cached) return cached;

  const entityMap = doc.entity ?? {};
  const graphs = doc.graphs ?? {};
  const links = doc.links ?? [];
  const eventMap = doc.event ?? {};

  const sortedEntities = Object.entries(entityMap).sort(([a], [b]) =>
    a.localeCompare(b),
  );

  const isEntityLink = (link: WeftLink) =>
    graphs[link.graph]?.domain !== "event" &&
    link.from in entityMap &&
    link.to in entityMap;

  // 1. 实体关系图筛选清单（仅包含连接实体的 link 图）
  const graphNames = new Set<string>();
  for (const link of links) {
    if (isEntityLink(link)) {
      graphNames.add(link.graph);
    }
  }
  const linkSections = [
    [...graphNames]
      .sort((a, b) => a.localeCompare(b))
      .map((g) => ({ v: g, label: g })),
  ];

  // 2. 事件时间截面选项（按发生时间升序，取事件完成时刻 end_tick ?? start_tick 作为切片刻度）
  const eventSliceTicks = new Map<string, number>();
  const chronologicalEvents = Object.entries(eventMap)
    .filter(([, ev]) => ev.start_tick !== undefined)
    .sort(
      ([idA, a], [idB, b]) =>
        (a.start_tick ?? 0) - (b.start_tick ?? 0) || idA.localeCompare(idB),
    );
  for (const [evId, ev] of chronologicalEvents) {
    eventSliceTicks.set(evId, ev.end_tick ?? ev.start_tick ?? 0);
  }
  const timeSections = [
    chronologicalEvents.map(([evId]) => ({
      v: evId,
      label: evId,
    })),
  ];

  // 3. 实体参与的关系边索引
  const linksByEntity = new Map<string, EntityLinkEntry[]>();
  const pushLink = (entityId: string, entry: EntityLinkEntry) => {
    let list = linksByEntity.get(entityId);
    if (!list) {
      list = [];
      linksByEntity.set(entityId, list);
    }
    list.push(entry);
  };

  for (let i = 0; i < links.length; i++) {
    const link = links[i];
    if (!isEntityLink(link)) continue;
    const timeRange = formatLinkInterval(link);
    const description = link.description ?? undefined;
    const label = link.label ?? undefined;
    const startTick = link.start?.tick;
    const endTick = link.end?.tick;

    pushLink(link.from, {
      key: `${link.graph}:${i}:from`,
      graph: link.graph,
      label,
      direction: link.directed ? "outgoing" : "symmetric",
      peerId: link.to,
      peerName: entityMap[link.to].id,
      startTick,
      endTick,
      timeRange,
      description,
    });
    if (link.from !== link.to) {
      pushLink(link.to, {
        key: `${link.graph}:${i}:to`,
        graph: link.graph,
        label,
        direction: link.directed ? "incoming" : "symmetric",
        peerId: link.from,
        peerName: entityMap[link.from].id,
        startTick,
        endTick,
        timeRange,
        description,
      });
    }
  }

  // 4. 实体关联事件索引
  const rawEventsByEntity = new Map<
    string,
    Array<EntityEventEntry & { tick: number }>
  >();
  for (const [id, ev] of Object.entries(eventMap)) {
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

  // 5. 等价关系商图分量
  const { hasEquivalence, repByEntity, membersByRep } =
    computeEquivalenceClasses(entityMap, links, graphs);

  const index: EntityDocIndex = {
    sortedEntities,
    linkSections,
    timeSections,
    eventSliceTicks,
    linksByEntity,
    eventsByEntity,
    hasEquivalence,
    repByEntity,
    membersByRep,
  };
  entityIndexCache.set(doc, index);
  return index;
}

function buildGraphEdges(
  keyPrefix: string,
  links: readonly WeftLink[],
  visibleSet: ReadonlySet<string>,
  mapNode: (id: string) => string,
  graphs: Readonly<Record<string, NonNullable<WeftDoc["graphs"]>[string]>>,
  dropEquivalenceSelf: boolean,
): Edge[] {
  type EdgeRecord = {
    index: number;
    source: string;
    target: string;
    label?: string;
    directed: boolean;
    timeRange?: string;
    description?: string;
    pairKey: string;
    canonicalForward: boolean;
  };

  const valid: EdgeRecord[] = [];
  const dedup = new Set<string>();

  for (let i = 0; i < links.length; i++) {
    const link = links[i];
    const source = mapNode(link.from);
    const target = mapNode(link.to);
    if (
      dropEquivalenceSelf &&
      graphs[link.graph]?.equivalence &&
      source === target
    ) {
      continue;
    }
    if (
      source === target ||
      !visibleSet.has(source) ||
      !visibleSet.has(target)
    ) {
      continue;
    }
    const timeRange = formatLinkInterval(link);
    const label = link.label ?? undefined;
    const canonicalForward = source < target;
    const pairKey = canonicalForward
      ? `${source}\0${target}`
      : `${target}\0${source}`;
    const dedupKey = link.directed
      ? `${source}\0${target}\0${label ?? ""}\0${timeRange ?? ""}`
      : `${pairKey}\0${label ?? ""}\0${timeRange ?? ""}`;
    if (dedup.has(dedupKey)) continue;
    dedup.add(dedupKey);

    valid.push({
      index: i,
      source,
      target,
      label,
      directed: link.directed,
      timeRange,
      description: link.description ?? undefined,
      pairKey,
      canonicalForward,
    });
  }

  const pairCounts = new Map<string, number>();
  for (const item of valid) {
    pairCounts.set(item.pairKey, (pairCounts.get(item.pairKey) ?? 0) + 1);
  }
  const pairSeen = new Map<string, number>();

  return valid.map((item) => {
    const pairIndex = pairSeen.get(item.pairKey) ?? 0;
    pairSeen.set(item.pairKey, pairIndex + 1);
    const data: FloatingEdgeData = {
      label: item.label,
      directed: item.directed,
      timeRange: item.timeRange,
      description: item.description,
      pairIndex,
      pairTotal: pairCounts.get(item.pairKey) ?? 1,
      canonicalForward: item.canonicalForward,
    };
    return {
      id: `${keyPrefix}:${item.index}`,
      type: "floating",
      source: item.source,
      target: item.target,
      markerStart: item.directed ? undefined : DIRECTED_MARKER,
      markerEnd: DIRECTED_MARKER,
      data,
    };
  });
}

export function buildEntityDetail(
  doc: WeftDoc | null,
  lang: Lang,
  entityId: string,
  viewMode: EntityViewMode = "narrative",
  activeEvent: string | null = null,
): EntityDetail | null {
  const entityMap = doc?.entity;
  if (!doc || !entityMap) return null;

  const index = getEntityDocIndex(doc);
  const resolvedId =
    viewMode === "quotient"
      ? (index.repByEntity.get(entityId) ?? entityId)
      : entityId;
  const entity = entityMap[resolvedId];
  if (!entity) return null;

  const sliceTick =
    activeEvent !== null ? index.eventSliceTicks.get(activeEvent) : undefined;

  const t = strings[lang];
  const members =
    viewMode === "quotient"
      ? (index.membersByRep.get(resolvedId) ?? [resolvedId])
      : [resolvedId];
  const aliases = members.filter((id) => id !== resolvedId);

  const mergedProps: Record<string, unknown> = {};
  const mergedEvents = new Map<string, EntityEventEntry>();
  const mergedLinks: EntityLinkEntry[] = [];
  const seenLinks = new Set<string>();
  const graphs = doc.graphs ?? {};

  for (const memberId of members) {
    const ent = entityMap[memberId];
    if (ent?.extra_props) {
      for (const [k, v] of Object.entries(ent.extra_props)) {
        if (!(k in mergedProps)) mergedProps[k] = v;
      }
    }
    for (const ev of index.eventsByEntity.get(memberId) ?? []) {
      if (!mergedEvents.has(ev.id)) mergedEvents.set(ev.id, ev);
    }
    for (const lk of index.linksByEntity.get(memberId) ?? []) {
      if (
        sliceTick !== undefined &&
        !isSpanActiveAtTick(lk.startTick, lk.endTick, sliceTick)
      ) {
        continue;
      }
      const peerId =
        viewMode === "quotient"
          ? (index.repByEntity.get(lk.peerId) ?? lk.peerId)
          : lk.peerId;
      if (
        viewMode === "quotient" &&
        peerId === resolvedId &&
        graphs[lk.graph]?.equivalence
      ) {
        continue;
      }
      const dedupKey = `${lk.graph}|${lk.label ?? ""}|${lk.direction}|${peerId}|${lk.timeRange ?? ""}`;
      if (seenLinks.has(dedupKey)) continue;
      seenLinks.add(dedupKey);
      mergedLinks.push({
        ...lk,
        peerId,
        peerName: entityMap[peerId]?.id ?? peerId,
      });
    }
  }

  const descriptions = members
    .map((m) => entityMap[m]?.description)
    .filter((d): d is string => Boolean(d && d.trim().length > 0));
  const baseTimeHumanizer = members
    .map((m) => entityMap[m]?.base_time_humanizer)
    .find((v): v is string => v !== undefined);
  const avatarPath = members
    .map((m) => entityMap[m]?.avatar)
    .find((v): v is string => Boolean(v));

  return {
    id: resolvedId,
    name: entity.id,
    aliases,
    avatar: avatarPath ? convertFileSrc(avatarPath) : undefined,
    baseTime:
      baseTimeHumanizer !== undefined
        ? `${t.baseTime}: ${baseTimeHumanizer}`
        : undefined,
    description:
      descriptions.length > 0 ? descriptions.join("\n\n") : undefined,
    extraProps: formatExtraProps(mergedProps),
    links: mergedLinks,
    events: [...mergedEvents.values()],
  };
}

const EMPTY_DOC_INDEX: EntityDocIndex = {
  sortedEntities: [],
  linkSections: [[]],
  timeSections: [[]],
  eventSliceTicks: new Map(),
  linksByEntity: new Map(),
  eventsByEntity: new Map(),
  hasEquivalence: false,
  repByEntity: new Map(),
  membersByRep: new Map(),
};

export function buildEntityView(
  doc: WeftDoc | null,
  lang: Lang,
  selectedLink: string | null,
  selectedEvent: string | null,
  viewMode: EntityViewMode,
  saved: Readonly<Record<string, Point>>,
) {
  const t = strings[lang];
  const entityMap = doc?.entity ?? {};
  const graphs = doc?.graphs ?? {};
  const allLinks = doc?.links ?? [];
  const index = doc ? getEntityDocIndex(doc) : EMPTY_DOC_INDEX;
  const {
    sortedEntities,
    linkSections,
    timeSections,
    eventSliceTicks,
    hasEquivalence,
    repByEntity,
    membersByRep,
  } = index;

  const isQuotient = viewMode === "quotient" && hasEquivalence;
  const mapNode = (id: string) =>
    isQuotient ? (repByEntity.get(id) ?? id) : id;

  const validGroupSet = new Set(
    linkSections.flatMap((s) => s.map((item) => item.v)),
  );
  const activeLink =
    selectedLink && validGroupSet.has(selectedLink) ? selectedLink : null;

  const activeEvent =
    selectedEvent && eventSliceTicks.has(selectedEvent) ? selectedEvent : null;
  const sliceTick =
    activeEvent !== null ? eventSliceTicks.get(activeEvent) : undefined;

  // 构建当前视角下的实体列表（上帝视角合并等价类）
  const baseEntities: Array<[string, EntityT, string[]]> = [];
  if (isQuotient) {
    const seenReps = new Set<string>();
    for (const [id] of sortedEntities) {
      const rep = mapNode(id);
      if (seenReps.has(rep)) continue;
      seenReps.add(rep);
      const repEntity = entityMap[rep];
      if (!repEntity) continue;
      const members = membersByRep.get(rep) ?? [rep];
      baseEntities.push([rep, repEntity, members]);
    }
  } else {
    for (const [id, ent] of sortedEntities) {
      baseEntities.push([id, ent, [id]]);
    }
  }

  const scopedLinks = activeLink
    ? allLinks.filter((l) => {
        if (l.graph !== activeLink) return false;
        if (!(l.from in entityMap) || !(l.to in entityMap)) return false;
        if (sliceTick !== undefined) {
          return isSpanActiveAtTick(l.start?.tick, l.end?.tick, sliceTick);
        }
        return true;
      })
    : [];

  const linkMembers = new Set<string>();
  for (const link of scopedLinks) {
    linkMembers.add(mapNode(link.from));
    linkMembers.add(mapNode(link.to));
  }

  const filtered = baseEntities.filter(([id]) => {
    if (activeLink) {
      return linkMembers.has(id);
    }
    return true;
  });

  const isHierarchy = activeLink ? Boolean(graphs[activeLink]?.acyclic) : false;

  const layoutMode: EntityLayoutMode = isHierarchy
    ? "hierarchy"
    : activeLink
      ? "network"
      : "grid";
  const useSavedPositions =
    layoutMode === "grid" && activeEvent === null && !isQuotient;

  const visibleIds = filtered.map(([id]) => id);
  const visibleSet = new Set(visibleIds);
  const edges = activeLink
    ? buildGraphEdges(
        `grp:${activeLink}:${activeEvent ?? "all"}`,
        scopedLinks,
        visibleSet,
        mapNode,
        graphs,
        isQuotient,
      )
    : [];

  const auto = gridLayout(visibleIds, EMPTY_SIZES);
  const nodes: EntityFlowNode[] = filtered.map(([id, e, members]) => {
    const aliases = members.filter((m) => m !== id);
    const mergedProps: Record<string, unknown> = {};
    for (const m of members) {
      const ent = entityMap[m];
      if (ent?.extra_props) {
        for (const [k, v] of Object.entries(ent.extra_props)) {
          if (!(k in mergedProps)) mergedProps[k] = v;
        }
      }
    }
    const avatarPath = members
      .map((m) => entityMap[m]?.avatar)
      .find((v): v is string => Boolean(v));
    const descriptions = members
      .map((m) => entityMap[m]?.description)
      .filter((d): d is string => Boolean(d && d.trim().length > 0));
    const baseTimeHumanizer = members
      .map((m) => entityMap[m]?.base_time_humanizer)
      .find((v): v is string => v !== undefined);

    return {
      id,
      type: "entity",
      position: (useSavedPositions ? saved[id] : undefined) ??
        auto[id] ?? { x: 0, y: 0 },
      data: {
        id,
        name: e.id,
        aliases,
        avatar: avatarPath ? convertFileSrc(avatarPath) : undefined,
        baseTime:
          baseTimeHumanizer !== undefined
            ? `${t.baseTime}: ${baseTimeHumanizer}`
            : undefined,
        extraProps: formatExtraProps(mergedProps),
        description:
          descriptions.length > 0 ? descriptions.join("\n\n") : undefined,
      },
    };
  });

  return {
    nodes,
    edges,
    layoutMode,
    useSavedPositions,
    activeLink,
    activeEvent,
    linkSections,
    timeSections,
    hasEquivalence,
  };
}
