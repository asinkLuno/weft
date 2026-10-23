import type { EventDefinition, EventTiming, WeftDoc } from "@/lib/document";
export type EventItem = EventDefinition &
  EventTiming & { id: string; title: string };

export function eventItems(doc: WeftDoc | null): EventItem[] {
  return Object.entries(doc?.event ?? {})
    .map(([id, event]) => ({
      ...event,
      ...doc?.event_times?.[id],
      id,
      title: id,
    }))
    .sort((a, b) => (a.start_tick ?? 0) - (b.start_tick ?? 0));
}

export type GanttBounds = {
  start: number;
  end: number;
  startDisplay?: string;
  endDisplay?: string;
};

export type GanttScale = {
  position: (tick: number) => number;
  breaks: Array<{ start: number; end: number; label: string }>;
};

export function ganttScale(events: EventItem[], origin: number): GanttScale {
  const intervals: Array<{ start: number; end: number }> = [];
  const labels = new Map<number, string>();
  for (const event of events) {
    if (event.start_tick === undefined) continue;
    const start = event.start_tick - origin;
    const end = (event.end_tick ?? event.start_tick) - origin;
    labels.set(start, event.start_display ?? "");
    labels.set(end, event.end_display ?? event.start_display ?? "");
    intervals.push({ start, end });
  }
  intervals.sort((a, b) => a.start - b.start);
  const merged: typeof intervals = [];
  for (const interval of intervals) {
    const previous = merged[merged.length - 1];
    if (previous && interval.start <= previous.end) {
      previous.end = Math.max(previous.end, interval.end);
    } else {
      merged.push({ ...interval });
    }
  }
  const activeTicks = merged.reduce(
    (sum, interval) => sum + interval.end - interval.start,
    0,
  );
  const gaps = merged
    .slice(1)
    .map((interval, index) => interval.start - merged[index].end);
  // 所有空白合计最多占活动时长的 15%；只有时间点时保留相对间距。
  const gapLimit =
    activeTicks > 0 && gaps.length > 0
      ? (activeTicks * 0.15) / gaps.length
      : Number.POSITIVE_INFINITY;
  const segments: Array<{
    start: number;
    end: number;
    offset: number;
    length: number;
  }> = [];
  const compressed: Array<{ start: number; end: number; label: string }> = [];
  let total = 0;
  for (const [index, interval] of merged.entries()) {
    if (index > 0) {
      const start = merged[index - 1].end;
      const end = interval.start;
      const length = Math.min(end - start, gapLimit);
      segments.push({ start, end, offset: total, length });
      if (length < end - start) {
        compressed.push({
          start: total,
          end: total + length,
          label: `${labels.get(start)} – ${labels.get(end)}`,
        });
      }
      total += length;
    }
    const length = interval.end - interval.start;
    segments.push({ ...interval, offset: total, length });
    total += length;
  }
  const span = Math.max(total, 1);
  return {
    position: (tick) => {
      const relative = tick - origin;
      const segment = segments.find(
        (segment) => relative >= segment.start && relative <= segment.end,
      );
      if (!segment) return relative <= 0 ? 0 : 100;
      const fraction =
        segment.end > segment.start
          ? (relative - segment.start) / (segment.end - segment.start)
          : 0;
      return ((segment.offset + fraction * segment.length) / span) * 100;
    },
    breaks: compressed.map((gap) => ({
      ...gap,
      start: (gap.start / span) * 100,
      end: (gap.end / span) * 100,
    })),
  };
}

export function buildNarrativeView(doc: WeftDoc | null) {
  // 事件索引：id → 展示数据
  const events = eventItems(doc);
  const byId = new Map(events.map((event) => [event.id, event]));
  let first: EventItem | undefined;
  let last: EventItem | undefined;
  for (const event of events) {
    if (event.start_tick === undefined) continue;
    if (!first || event.start_tick < (first.start_tick ?? 0)) first = event;
    const end = event.end_tick ?? event.start_tick;
    if (!last || end > (last.end_tick ?? last.start_tick ?? 0)) last = event;
  }
  const bounds: GanttBounds = {
    start: first?.start_tick ?? 0,
    end: last?.end_tick ?? last?.start_tick ?? 0,
    startDisplay: first?.start_display,
    endDisplay: last?.end_display ?? last?.start_display,
  };
  const scale = ganttScale(events, bounds.start);
  const entries = Object.entries(doc?.narrative ?? {})
    .filter(([, nar]) => nar.length > 0)
    .sort(([a], [b]) => a.localeCompare(b));
  return { byId, bounds, scale, entries };
}
