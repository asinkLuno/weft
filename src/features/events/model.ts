import { convertFileSrc } from "@tauri-apps/api/core";
import type { Event, Events, WeftDoc } from "@/lib/document";
import { formatTimeRange } from "@/lib/ui";

export type EventItem = Event & { id: string };

export type EventEntityDelta = {
  start: string;
  /** 仅当结束差值存在且不同于开始差值时有值 */
  end?: string;
};

export type EventEntityRef = {
  id: string;
  name: string;
  avatar?: string;
  description?: string;
  baseTimeDelta?: EventEntityDelta;
};

export type EventPlotLink = {
  key: string;
  graph: string;
  label?: string;
  direction: "symmetric" | "outgoing" | "incoming";
  peerId: string;
  peerTime?: string;
  description?: string;
};

export type EventDetail = {
  id: string;
  time?: string;
  description?: string;
  entities: EventEntityRef[];
  links: EventPlotLink[];
  lists: Array<{ id: string; label: string }>;
  prevId?: string;
  nextId?: string;
};

export type EventsIndex = {
  listsByEvent: ReadonlyMap<string, Array<{ id: string; label: string }>>;
  listIndexByName: ReadonlyMap<string, ReadonlyMap<string, number>>;
  plotLinksByEvent: ReadonlyMap<string, EventPlotLink[]>;
};

export type GanttBreak = {
  start: number;
  end: number;
};

export type TickSegmentRequest = {
  start: number;
  end: number;
  count: number;
};

export type CalendarTick = {
  tick: number;
  label: string;
};

export type GanttChartModel = {
  events: EventItem[];
  minTick: number;
  maxTick: number;
  startDisplay?: string;
  endDisplay?: string;
  breaks: GanttBreak[];
  initialLabels: Map<number, string>;
};

export function buildEventsView(doc: WeftDoc | null) {
  const eventMap = doc?.event ?? {};
  const graphs = doc?.graphs ?? {};
  const allLinks = doc?.links ?? [];

  const byId = new Map<string, EventItem>(
    Object.entries(eventMap).map(([id, event]) => [id, { ...event, id }]),
  );
  const entries = Object.entries(doc?.events ?? {})
    .filter(([, ids]) => ids.length > 0)
    .sort(([a], [b]) => a.localeCompare(b));

  const listsByEvent = new Map<string, Array<{ id: string; label: string }>>();
  const listIndexByName = new Map<string, Map<string, number>>();
  for (const [listName, ids] of entries) {
    const posMap = new Map<string, number>();
    for (let i = 0; i < ids.length; i++) {
      const eventId = ids[i];
      if (!posMap.has(eventId)) {
        posMap.set(eventId, i);
        let list = listsByEvent.get(eventId);
        if (!list) {
          list = [];
          listsByEvent.set(eventId, list);
        }
        list.push({ id: listName, label: listName });
      }
    }
    listIndexByName.set(listName, posMap);
  }

  const plotLinksByEvent = new Map<string, EventPlotLink[]>();
  const pushPlotLink = (eventId: string, entry: EventPlotLink) => {
    let list = plotLinksByEvent.get(eventId);
    if (!list) {
      list = [];
      plotLinksByEvent.set(eventId, list);
    }
    list.push(entry);
  };

  for (let i = 0; i < allLinks.length; i++) {
    const link = allLinks[i];
    if (graphs[link.graph]?.domain === "entity") continue;
    const fromEv = eventMap[link.from];
    const toEv = eventMap[link.to];
    if (!fromEv || !toEv) continue;

    const label = link.label ?? undefined;
    const description = link.description ?? undefined;

    pushPlotLink(link.from, {
      key: `${link.graph}:${i}:from`,
      graph: link.graph,
      label,
      direction: link.directed ? "outgoing" : "symmetric",
      peerId: link.to,
      peerTime: formatTimeRange(toEv.start_display, toEv.end_display),
      description,
    });
    if (link.from !== link.to) {
      pushPlotLink(link.to, {
        key: `${link.graph}:${i}:to`,
        graph: link.graph,
        label,
        direction: link.directed ? "incoming" : "symmetric",
        peerId: link.from,
        peerTime: formatTimeRange(fromEv.start_display, fromEv.end_display),
        description,
      });
    }
  }

  const calendarId = doc?.story?.date.id ?? "gregorian";
  return {
    byId,
    entries,
    calendarId,
    listsByEvent,
    listIndexByName,
    plotLinksByEvent,
  };
}

function resolveEventNeighbors(
  eventsMap: WeftDoc["events"],
  eventId: string,
  activeListName: string | null,
  lists: ReadonlyArray<{ id: string; label: string }>,
  listIndexByName: ReadonlyMap<string, ReadonlyMap<string, number>>,
): { prevId?: string; nextId?: string } {
  const activeIdx = activeListName
    ? listIndexByName.get(activeListName)?.get(eventId)
    : undefined;
  const navListName =
    activeIdx !== undefined ? activeListName : (lists[0]?.id ?? null);
  if (!navListName) return {};
  const idx =
    activeIdx !== undefined
      ? activeIdx
      : listIndexByName.get(navListName)?.get(eventId);
  if (idx === undefined) return {};
  const navIds = eventsMap?.[navListName] ?? [];
  return {
    prevId: navIds[idx - 1],
    nextId: navIds[idx + 1],
  };
}

export function buildEventDetail(
  doc: WeftDoc | null,
  eventId: string,
  activeListName: string | null,
  index: EventsIndex,
): EventDetail | null {
  const event = doc?.event?.[eventId];
  if (!event) return null;

  const entityMap = doc?.entity ?? {};
  const entityDeltas = event.entity_deltas;
  const entities: EventEntityRef[] = (event.entities ?? []).map((id) => {
    const entity = entityMap[id];
    const delta = entityDeltas?.[id];
    const baseTimeDelta = delta
      ? {
          start: delta.start_display,
          end:
            delta.end_display && delta.end_display !== delta.start_display
              ? delta.end_display
              : undefined,
        }
      : undefined;
    return {
      id,
      name: entity?.id ?? id,
      avatar: entity?.avatar ? convertFileSrc(entity.avatar) : undefined,
      description: entity?.description,
      baseTimeDelta,
    };
  });

  const links = index.plotLinksByEvent.get(eventId) ?? [];
  const lists = index.listsByEvent.get(eventId) ?? [];
  const { prevId, nextId } = resolveEventNeighbors(
    doc?.events,
    eventId,
    activeListName,
    lists,
    index.listIndexByName,
  );

  return {
    id: eventId,
    time: formatTimeRange(event.start_display, event.end_display),
    description: event.description,
    entities,
    links,
    lists,
    prevId,
    nextId,
  };
}

/**
 * 为单个事件列表构造 ECharts 6 甘特图所需的坐标边界、断轴区间与已知刻度文案。
 * 事件展示顺序保持作者在 `events` 列表里声明的顺序。
 */
export function buildGanttChartModel(
  ids: Events,
  byId: Map<string, EventItem>,
): GanttChartModel {
  const events: EventItem[] = [];
  const initialLabels = new Map<number, string>();
  const intervals: Array<{ start: number; end: number }> = [];
  let first: EventItem | undefined;
  let last: EventItem | undefined;

  for (const id of ids) {
    const event = byId.get(id);
    if (!event) continue;
    events.push(event);
    if (event.start_tick === undefined) continue;
    const start = event.start_tick;
    const end = event.end_tick ?? start;
    if (event.start_display) {
      initialLabels.set(start, event.start_display);
    }
    if (event.end_tick !== undefined && event.end_display) {
      initialLabels.set(event.end_tick, event.end_display);
    }
    if (!first || start < (first.start_tick ?? 0)) {
      first = event;
    }
    if (!last || end > (last.end_tick ?? last.start_tick ?? 0)) {
      last = event;
    }
    intervals.push({ start, end });
  }

  const rawMin = first?.start_tick ?? 0;
  const rawMax = last?.end_tick ?? last?.start_tick ?? rawMin;
  const totalSpan = rawMax - rawMin;

  if (totalSpan <= 0) {
    return {
      events,
      minTick: rawMin - 1,
      maxTick: rawMax + 1,
      startDisplay: first?.start_display,
      endDisplay: last?.end_display ?? last?.start_display,
      breaks: [],
      initialLabels,
    };
  }

  intervals.sort((a, b) => a.start - b.start);
  const merged: Array<{ start: number; end: number }> = [];
  for (const interval of intervals) {
    const prev = merged[merged.length - 1];
    if (prev && interval.start <= prev.end) {
      prev.end = Math.max(prev.end, interval.end);
    } else {
      merged.push({ ...interval });
    }
  }

  const activeSpan = merged.reduce(
    (sum, interval) => sum + (interval.end - interval.start),
    0,
  );
  const gaps: Array<{ start: number; end: number; length: number }> = [];
  for (let i = 1; i < merged.length; i += 1) {
    const start = merged[i - 1].end;
    const end = merged[i].start;
    if (end > start) {
      gaps.push({ start, end, length: end - start });
    }
  }

  // 仅折叠占比显著的大段空白，避免把正常分布的密集事件切得支离破碎。
  const foldedGaps = gaps.filter(
    (gap) =>
      gap.length >= totalSpan * 0.35 ||
      (gap.length >= totalSpan * 0.25 &&
        activeSpan > 0 &&
        activeSpan < totalSpan * 0.05 &&
        gaps.length <= 4),
  );

  const foldedTotal = foldedGaps.reduce((sum, gap) => sum + gap.length, 0);
  const unbrokenSpan = Math.max(totalSpan - foldedTotal, 0);
  const referenceSpan =
    unbrokenSpan > 0 ? unbrokenSpan : Math.max(1, Math.round(totalSpan * 0.02));

  // 在断轴两端保留小段留白，避免锯齿贴到事件条/里程碑点上。
  const breaks: GanttBreak[] = [];
  for (const gap of foldedGaps) {
    const maxInset = Math.floor(gap.length * 0.4);
    const desiredInset = Math.max(1, Math.round(referenceSpan * 0.06));
    const inset = Math.min(desiredInset, maxInset);
    const start = gap.start + inset;
    const end = gap.end - inset;
    if (end > start) {
      breaks.push({ start, end });
    }
  }

  const edgePad = Math.max(1, Math.round(referenceSpan * 0.04));

  return {
    events,
    minTick: rawMin - edgePad,
    maxTick: rawMax + edgePad,
    startDisplay: first?.start_display,
    endDisplay: last?.end_display ?? last?.start_display,
    breaks,
    initialLabels,
  };
}

/**
 * 将当前可视区间按未展开的断轴切分成若干连续可视段，按长度比例分配刻度预算。
 */
export function buildTickSegments(
  viewMin: number,
  viewMax: number,
  activeBreaks: GanttBreak[],
  totalBudget: number,
): TickSegmentRequest[] {
  const start = Math.floor(viewMin);
  const end = Math.ceil(viewMax);
  if (end <= start) {
    return [{ start, end: start, count: 1 }];
  }

  const clipped: Array<{ start: number; end: number }> = [];
  for (const brk of activeBreaks) {
    if (brk.end <= start || brk.start >= end) continue;
    const clippedStart = Math.max(start, Math.floor(brk.start));
    const clippedEnd = Math.min(end, Math.ceil(brk.end));
    if (clippedEnd > clippedStart) {
      clipped.push({ start: clippedStart, end: clippedEnd });
    }
  }
  clipped.sort((a, b) => a.start - b.start);

  const rawSegments: Array<{ start: number; end: number }> = [];
  let cursor = start;
  for (const brk of clipped) {
    if (brk.start > cursor) {
      rawSegments.push({ start: cursor, end: brk.start });
    }
    cursor = Math.max(cursor, brk.end);
  }
  if (cursor < end) {
    rawSegments.push({ start: cursor, end });
  }
  if (rawSegments.length === 0) {
    return [{ start, end, count: Math.max(2, totalBudget) }];
  }

  const visibleSpan = rawSegments.reduce(
    (sum, seg) => sum + Math.max(1, seg.end - seg.start),
    0,
  );
  const budget = Math.max(rawSegments.length * 2, totalBudget);

  return rawSegments.map((seg) => {
    const weight = Math.max(1, seg.end - seg.start) / visibleSpan;
    const count = Math.max(2, Math.round(budget * weight));
    return { start: seg.start, end: seg.end, count };
  });
}
