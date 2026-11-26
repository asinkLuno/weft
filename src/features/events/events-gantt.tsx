import { invoke } from "@tauri-apps/api/core";
import { CustomChart } from "echarts/charts";
import {
  DataZoomComponent,
  GridComponent,
  TooltipComponent,
} from "echarts/components";
import * as echarts from "echarts/core";
import { AxisBreak } from "echarts/features";
import { CanvasRenderer } from "echarts/renderers";
import { FoldHorizontal, RotateCcw, UnfoldHorizontal } from "lucide-react";
import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { Lang, strings } from "@/i18n";
import type { Events } from "@/lib/document";
import {
  breakKey,
  buildGanttOption,
  MAX_VISIBLE_ROWS,
  ROW_HEIGHT,
  readDaisyPalette,
} from "./gantt-option";
import {
  buildGanttChartModel,
  buildTickSegments,
  type CalendarTick,
  type EventItem,
  type GanttBreak,
} from "./model";

echarts.use([
  CustomChart,
  GridComponent,
  TooltipComponent,
  DataZoomComponent,
  AxisBreak,
  CanvasRenderer,
]);

// 按「历法 + 语言」共享刻度文案缓存，跨事件列表与缩放往返零延迟命中。
const MAX_CACHED_TICKS = 2000;
let sharedCacheKey = "";
const sharedTickLabels = new Map<number, string>();

function getSharedTickCache(calendarKey: string): Map<number, string> {
  if (sharedCacheKey !== calendarKey) {
    sharedCacheKey = calendarKey;
    sharedTickLabels.clear();
  }
  return sharedTickLabels;
}

function storeTickLabel(
  cache: Map<number, string>,
  tick: number,
  label: string,
): void {
  if (cache.size >= MAX_CACHED_TICKS && !cache.has(tick)) {
    const oldest = cache.keys().next().value;
    if (oldest !== undefined) cache.delete(oldest);
  }
  cache.set(tick, label);
}

const FOLD_ICON = <FoldHorizontal className="size-3.5" />;
const UNFOLD_ICON = <UnfoldHorizontal className="size-3.5" />;
const RESET_ICON = <RotateCcw className="size-3.5" />;

export const EventsGantt = memo(function EventsGantt({
  name,
  ids,
  byId,
  calendarKey,
  t,
  onSelectEvent,
}: {
  name: string;
  ids: Events;
  byId: Map<string, EventItem>;
  calendarKey: string;
  t: (typeof strings)[Lang];
  onSelectEvent: (eventId: string, listName: string) => void;
}) {
  const model = useMemo(() => buildGanttChartModel(ids, byId), [ids, byId]);
  const breakEdges = useMemo(() => {
    const ticks = model.breaks.flatMap((b) => [b.start, b.end]);
    return { ticks, tickSet: new Set(ticks) };
  }, [model.breaks]);
  const containerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<echarts.ECharts | null>(null);
  const onSelectEventRef = useRef(onSelectEvent);
  const expandedBreaksRef = useRef<Set<string>>(new Set());
  const viewRangeRef = useRef<{ min: number; max: number }>({
    min: model.minTick,
    max: model.maxTick,
  });
  const fetchTimerRef = useRef<number | null>(null);
  const requestSeqRef = useRef(0);

  useEffect(() => {
    onSelectEventRef.current = onSelectEvent;
  }, [onSelectEvent]);

  const [isZoomed, setIsZoomed] = useState(false);
  const [hasExpandedBreak, setHasExpandedBreak] = useState(false);

  const chartHeight = Math.max(
    180,
    Math.min(model.events.length, MAX_VISIBLE_ROWS) * ROW_HEIGHT + 96,
  );

  const cancelPendingRefresh = useCallback(() => {
    if (fetchTimerRef.current !== null) {
      window.clearTimeout(fetchTimerRef.current);
      fetchTimerRef.current = null;
    }
  }, []);

  const refreshCalendarTicks = useCallback(
    (rangeMin: number, rangeMax: number) => {
      cancelPendingRefresh();
      const width = containerRef.current?.clientWidth ?? 600;
      const budget = Math.max(4, Math.min(10, Math.floor(width / 120)));
      const activeBreaks: GanttBreak[] = model.breaks.filter(
        (brk) => !expandedBreaksRef.current.has(breakKey(brk)),
      );
      const segments = buildTickSegments(
        rangeMin,
        rangeMax,
        activeBreaks,
        budget,
      );
      const extraTicks = breakEdges.ticks;
      const extraSet = breakEdges.tickSet;
      const seq = ++requestSeqRef.current;

      void invoke<CalendarTick[]>("calendar_axis_ticks", {
        segments,
        extraTicks,
      })
        .then((result) => {
          if (seq !== requestSeqRef.current) return;
          if (!Array.isArray(result) || result.length === 0) return;

          const cache = getSharedTickCache(calendarKey);
          const axisTickValues: number[] = [];
          for (const item of result) {
            storeTickLabel(cache, item.tick, item.label);
            if (!extraSet.has(item.tick)) {
              axisTickValues.push(item.tick);
            }
          }
          const ticks =
            axisTickValues.length > 0
              ? axisTickValues
              : result.map((item) => item.tick);
          const chart = chartRef.current;
          if (!chart || chart.isDisposed()) return;
          chart.setOption({
            xAxis: {
              axisTick: { customValues: ticks },
              axisLabel: { customValues: ticks },
            },
          });
        })
        .catch(() => {
          // 离线或无后端历法会话时，保留事件自身文案作为降级显示。
        });
    },
    [breakEdges, calendarKey, cancelPendingRefresh, model.breaks],
  );

  const scheduleTickRefresh = useCallback(
    (rangeMin: number, rangeMax: number) => {
      cancelPendingRefresh();
      fetchTimerRef.current = window.setTimeout(() => {
        fetchTimerRef.current = null;
        refreshCalendarTicks(rangeMin, rangeMax);
      }, 100);
    },
    [cancelPendingRefresh, refreshCalendarTicks],
  );

  const scheduleRefreshRef = useRef(scheduleTickRefresh);
  useEffect(() => {
    scheduleRefreshRef.current = scheduleTickRefresh;
  }, [scheduleTickRefresh]);

  // 初始化与数据/语言变更时重建图配置，同时同步后端历法刻度。
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const cache = getSharedTickCache(calendarKey);
    for (const [tick, label] of model.initialLabels) {
      storeTickLabel(cache, tick, label);
    }
    expandedBreaksRef.current = new Set();
    viewRangeRef.current = { min: model.minTick, max: model.maxTick };
    setIsZoomed(false);
    setHasExpandedBreak(false);

    let chart = chartRef.current;
    if (!chart || chart.isDisposed()) {
      chart = echarts.init(container, undefined, { renderer: "canvas" });
      chartRef.current = chart;
    }

    const fallbackTicks = Array.from(model.initialLabels.keys()).sort(
      (a, b) => a - b,
    );
    const option = buildGanttOption({
      name,
      model,
      palette: readDaisyPalette(container),
      entitiesLabel: t.ganttEntities,
      formatExactTick: (value) => {
        const labels = getSharedTickCache(calendarKey);
        return labels.get(value) ?? labels.get(Math.round(value)) ?? "";
      },
      customValues: fallbackTicks,
      expandedBreaks: expandedBreaksRef.current,
    });
    chart.setOption(option, true);
    refreshCalendarTicks(model.minTick, model.maxTick);

    const handleDataZoom = () => {
      if (!chart || chart.isDisposed()) return;
      const opt = chart.getOption() as {
        dataZoom?: Array<{
          xAxisIndex?: number | number[];
          start?: number;
          end?: number;
          startValue?: number;
          endValue?: number;
        }>;
      };
      const xZoom = opt.dataZoom?.find((dz) => {
        const idx = dz.xAxisIndex;
        return idx === 0 || (Array.isArray(idx) && idx.includes(0));
      });
      if (!xZoom) return;
      const startPct = xZoom.start ?? 0;
      const endPct = xZoom.end ?? 100;
      setIsZoomed(startPct > 0.1 || endPct < 99.9);

      const span = model.maxTick - model.minTick;
      const nextMin =
        typeof xZoom.startValue === "number"
          ? xZoom.startValue
          : model.minTick + (span * startPct) / 100;
      const nextMax =
        typeof xZoom.endValue === "number"
          ? xZoom.endValue
          : model.minTick + (span * endPct) / 100;
      if (
        nextMin === viewRangeRef.current.min &&
        nextMax === viewRangeRef.current.max
      ) {
        return;
      }
      viewRangeRef.current = { min: nextMin, max: nextMax };
      scheduleRefreshRef.current(nextMin, nextMax);
    };

    const handleBreakChanged = (rawEvent: unknown) => {
      const event = rawEvent as {
        breaks?: Array<{ start: number; end: number; isExpanded?: boolean }>;
      };
      if (!Array.isArray(event?.breaks)) return;
      for (const brk of event.breaks) {
        const key = breakKey({ start: brk.start, end: brk.end });
        if (brk.isExpanded) {
          expandedBreaksRef.current.add(key);
        } else {
          expandedBreaksRef.current.delete(key);
        }
      }
      setHasExpandedBreak(expandedBreaksRef.current.size > 0);
      scheduleRefreshRef.current(
        viewRangeRef.current.min,
        viewRangeRef.current.max,
      );
    };

    const handleClick = (params: unknown) => {
      const click = params as {
        componentType?: string;
        dataIndex?: number;
        value?: unknown;
      };
      if (
        click.componentType === "series" &&
        typeof click.dataIndex === "number"
      ) {
        const event = model.events[click.dataIndex];
        if (event) onSelectEventRef.current(event.id, name);
      } else if (
        click.componentType === "yAxis" &&
        typeof click.value === "string"
      ) {
        onSelectEventRef.current(click.value, name);
      }
    };

    chart.on("datazoom", handleDataZoom);
    chart.on("axisbreakchanged", handleBreakChanged);
    chart.on("click", handleClick);

    return () => {
      chart?.off("datazoom", handleDataZoom);
      chart?.off("axisbreakchanged", handleBreakChanged);
      chart?.off("click", handleClick);
      cancelPendingRefresh();
    };
  }, [
    calendarKey,
    cancelPendingRefresh,
    model,
    name,
    refreshCalendarTicks,
    t.ganttEntities,
  ]);

  // 响应 Dockview 面板分屏/切换可见性与窗口尺寸变化；仅在挂载时绑定一次。
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    let lastWidth = container.clientWidth;

    const observer = new ResizeObserver((entries) => {
      const rect = entries[0]?.contentRect;
      if (!rect || rect.width <= 0 || rect.height <= 0) return;
      const chart = chartRef.current;
      if (!chart || chart.isDisposed()) return;
      chart.resize();
      if (Math.abs(rect.width - lastWidth) > 80) {
        lastWidth = rect.width;
        scheduleRefreshRef.current(
          viewRangeRef.current.min,
          viewRangeRef.current.max,
        );
      }
    });

    observer.observe(container);
    return () => {
      observer.disconnect();
    };
  }, []);

  // 卸载时释放 ECharts 实例。
  useEffect(() => {
    return () => {
      cancelPendingRefresh();
      if (chartRef.current && !chartRef.current.isDisposed()) {
        chartRef.current.dispose();
      }
      chartRef.current = null;
    };
  }, [cancelPendingRefresh]);

  const handleToggleBreaks = useCallback(() => {
    const chart = chartRef.current;
    if (!chart || chart.isDisposed() || model.breaks.length === 0) return;
    chart.dispatchAction({
      type: hasExpandedBreak ? "collapseAxisBreak" : "expandAxisBreak",
      xAxisIndex: 0,
      breaks: model.breaks.map((b) => ({ start: b.start, end: b.end })),
    });
  }, [hasExpandedBreak, model.breaks]);

  const handleResetView = useCallback(() => {
    const chart = chartRef.current;
    if (!chart || chart.isDisposed()) return;
    viewRangeRef.current = { min: model.minTick, max: model.maxTick };
    chart.dispatchAction({
      type: "dataZoom",
      dataZoomIndex: 0,
      start: 0,
      end: 100,
    });
    if (model.breaks.length > 0 && expandedBreaksRef.current.size > 0) {
      chart.dispatchAction({
        type: "collapseAxisBreak",
        xAxisIndex: 0,
        breaks: model.breaks.map((b) => ({ start: b.start, end: b.end })),
      });
    }
    setIsZoomed(false);
    refreshCalendarTicks(model.minTick, model.maxTick);
  }, [model.breaks, model.maxTick, model.minTick, refreshCalendarTicks]);

  return (
    <section className="min-w-0">
      <div className="mb-2 flex flex-wrap items-center justify-between gap-2">
        <div className="flex min-w-0 items-baseline gap-2">
          <h2 className="truncate text-base font-semibold">{name}</h2>
          <span className="badge badge-ghost badge-xs">
            {model.events.length}
          </span>
          {model.startDisplay ? (
            <span className="truncate font-mono text-xs text-base-content/50">
              {model.startDisplay}
              {model.endDisplay && model.endDisplay !== model.startDisplay
                ? ` – ${model.endDisplay}`
                : ""}
            </span>
          ) : null}
        </div>
        <div className="flex items-center gap-1">
          <span className="hidden text-xs text-base-content/40 sm:inline">
            {t.ganttZoomHint}
          </span>
          {model.breaks.length > 0 ? (
            <button
              type="button"
              className="btn btn-ghost btn-xs"
              onClick={handleToggleBreaks}
            >
              {hasExpandedBreak ? FOLD_ICON : UNFOLD_ICON}
              {hasExpandedBreak ? t.ganttCollapseBreaks : t.ganttExpandBreaks}
            </button>
          ) : null}
          {isZoomed || hasExpandedBreak ? (
            <button
              type="button"
              className="btn btn-ghost btn-xs"
              onClick={handleResetView}
            >
              {RESET_ICON}
              {t.ganttResetZoom}
            </button>
          ) : null}
        </div>
      </div>
      <div className="rounded-box border border-base-300 bg-base-100">
        <div
          ref={containerRef}
          className="w-full"
          style={{ height: `${chartHeight}px` }}
        />
      </div>
    </section>
  );
});
