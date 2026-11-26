import type { CustomSeriesOption } from "echarts/charts";
import type {
  DataZoomComponentOption,
  GridComponentOption,
  TooltipComponentOption,
} from "echarts/components";
import * as echarts from "echarts/core";
import type { EventItem, GanttBreak, GanttChartModel } from "./model";

export type GanttOption = echarts.ComposeOption<
  | CustomSeriesOption
  | GridComponentOption
  | TooltipComponentOption
  | DataZoomComponentOption
>;

export type ChartPalette = {
  primary: string;
  primarySoft: string;
  primaryBorder: string;
  base100: string;
  base200: string;
  base300: string;
  content: string;
  contentMuted: string;
  contentSubtle: string;
  fontSans: string;
  fontMono: string;
};

export const ROW_HEIGHT = 36;
export const MAX_VISIBLE_ROWS = 12;

const BAR_HEIGHT = 16;
const MIN_BAR_WIDTH = 8;
const DIAMOND_RADIUS = 6;

const HTML_ESCAPE_MAP: Record<string, string> = {
  "&": "&amp;",
  "<": "&lt;",
  ">": "&gt;",
  '"': "&quot;",
  "'": "&#39;",
};

let colorCanvasCtx: CanvasRenderingContext2D | null | undefined;

function getColorCanvasCtx(): CanvasRenderingContext2D | null {
  if (colorCanvasCtx !== undefined) return colorCanvasCtx;
  if (typeof document === "undefined") {
    colorCanvasCtx = null;
    return null;
  }
  const canvas = document.createElement("canvas");
  canvas.width = 1;
  canvas.height = 1;
  colorCanvasCtx = canvas.getContext("2d", { willReadFrequently: true });
  return colorCanvasCtx;
}

// ZRender 无法直接解析 daisyUI 5 的 oklch() 颜色，复用 1×1 Canvas 转成 sRGB rgba()。
function resolveCssColor(
  cssValue: string,
  fallback: string,
  alpha?: number,
): string {
  const raw = cssValue.trim() || fallback;
  const ctx = getColorCanvasCtx();
  if (!ctx) return fallback;
  ctx.clearRect(0, 0, 1, 1);
  ctx.fillStyle = fallback;
  ctx.fillStyle = raw;
  ctx.fillRect(0, 0, 1, 1);
  const [r, g, b, a] = ctx.getImageData(0, 0, 1, 1).data;
  const finalAlpha = alpha ?? Number((a / 255).toFixed(3));
  return `rgba(${r}, ${g}, ${b}, ${finalAlpha})`;
}

let cachedPaletteKey: string | null = null;
let cachedPalette: ChartPalette | null = null;

export function readDaisyPalette(element: HTMLElement): ChartPalette {
  const style = getComputedStyle(element);
  const rootStyle = getComputedStyle(document.documentElement);
  const readVar = (name: string) =>
    style.getPropertyValue(name).trim() ||
    rootStyle.getPropertyValue(name).trim();

  const primaryRaw = readVar("--color-primary");
  const base100Raw = readVar("--color-base-100");
  const base200Raw = readVar("--color-base-200");
  const base300Raw = readVar("--color-base-300");
  const contentRaw = readVar("--color-base-content");
  const fontSans = style.fontFamily || "sans-serif";

  const key = `${primaryRaw}|${base100Raw}|${base200Raw}|${base300Raw}|${contentRaw}|${fontSans}`;
  if (cachedPalette && cachedPaletteKey === key) {
    return cachedPalette;
  }

  cachedPaletteKey = key;
  cachedPalette = {
    primary: resolveCssColor(primaryRaw, "#002fa7", 1),
    primarySoft: resolveCssColor(primaryRaw, "#002fa7", 0.14),
    primaryBorder: resolveCssColor(primaryRaw, "#002fa7", 0.45),
    base100: resolveCssColor(base100Raw, "#ffffff", 1),
    base200: resolveCssColor(base200Raw, "#f5f5f5", 1),
    base300: resolveCssColor(base300Raw, "#e5e5e5", 1),
    content: resolveCssColor(contentRaw, "#1f2937", 1),
    contentMuted: resolveCssColor(contentRaw, "#1f2937", 0.65),
    contentSubtle: resolveCssColor(contentRaw, "#1f2937", 0.12),
    fontSans,
    fontMono:
      'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace',
  };
  return cachedPalette;
}

export function breakKey(brk: GanttBreak): string {
  return `${brk.start}:${brk.end}`;
}

function escapeHtml(text: string): string {
  return text.replace(/[&<>"']/g, (ch) => HTML_ESCAPE_MAP[ch] ?? ch);
}

function formatTooltipHtml(
  event: EventItem,
  palette: ChartPalette,
  entitiesLabel: string,
): string {
  const timeText = event.end_display
    ? `${event.start_display ?? ""} – ${event.end_display}`
    : (event.start_display ?? "");
  const entitiesHtml =
    event.entities && event.entities.length > 0
      ? `<div style="margin-top:8px;display:flex;flex-wrap:wrap;gap:4px;align-items:center;">
          <span style="font-size:11px;color:${palette.contentMuted};margin-right:2px;">${escapeHtml(entitiesLabel)}:</span>
          ${event.entities
            .map(
              (entity) =>
                `<span style="display:inline-block;padding:1px 6px;border-radius:4px;font-size:11px;background:${palette.base200};border:1px solid ${palette.base300};color:${palette.content};">${escapeHtml(entity)}</span>`,
            )
            .join("")}
        </div>`
      : "";
  const descHtml = event.description
    ? `<div style="margin-top:8px;white-space:pre-wrap;line-height:1.5;font-size:12px;color:${palette.content};opacity:0.85;">${escapeHtml(event.description)}</div>`
    : "";

  return `<div style="max-width:300px;font-family:${palette.fontSans};">
    <div style="font-size:13px;font-weight:600;color:${palette.content};">${escapeHtml(event.id)}</div>
    ${
      timeText
        ? `<div style="margin-top:2px;font-family:${palette.fontMono};font-size:11px;color:${palette.contentMuted};">${escapeHtml(timeText)}</div>`
        : ""
    }
    ${entitiesHtml}
    ${descHtml}
  </div>`;
}

function buildDataZoom(
  palette: ChartPalette,
  needsVerticalScroll: boolean,
): DataZoomComponentOption[] {
  const list: DataZoomComponentOption[] = [
    {
      type: "inside",
      xAxisIndex: 0,
      filterMode: "none",
      zoomOnMouseWheel: "ctrl",
      moveOnMouseMove: true,
      moveOnMouseWheel: false,
      preventDefaultMouseMove: false,
    },
    {
      type: "slider",
      xAxisIndex: 0,
      filterMode: "none",
      height: 20,
      bottom: 10,
      showDetail: false,
      borderColor: palette.base300,
      backgroundColor: palette.base200,
      fillerColor: palette.primarySoft,
      handleStyle: {
        color: palette.base100,
        borderColor: palette.primaryBorder,
      },
      moveHandleStyle: {
        color: palette.primaryBorder,
      },
      dataBackground: {
        lineStyle: { color: palette.primaryBorder, opacity: 0.5 },
        areaStyle: { color: palette.primarySoft },
      },
      selectedDataBackground: {
        lineStyle: { color: palette.primary, opacity: 0.8 },
        areaStyle: { color: palette.primarySoft },
      },
    },
  ];

  if (needsVerticalScroll) {
    list.push({
      type: "slider",
      yAxisIndex: 0,
      filterMode: "none",
      right: 6,
      width: 12,
      startValue: 0,
      endValue: MAX_VISIBLE_ROWS - 1,
      zoomLock: true,
      showDetail: false,
      brushSelect: false,
      borderColor: palette.base300,
      backgroundColor: palette.base200,
      fillerColor: palette.primarySoft,
      handleSize: 0,
    });
  }
  return list;
}

function createRenderItem(
  palette: ChartPalette,
): NonNullable<CustomSeriesOption["renderItem"]> {
  return (params, api) => {
    const rowIndex = Number(api.value(0));
    const startTick = Number(api.value(1));
    const endTick = Number(api.value(2));
    const isPoint = Number(api.value(3)) === 1;

    const startCoord = api.coord([startTick, rowIndex]);
    const endCoord = api.coord([endTick, rowIndex]);
    const coordSys = params.coordSys as unknown as {
      x: number;
      y: number;
      width: number;
      height: number;
    };

    if (isPoint) {
      const cx = startCoord[0];
      const cy = startCoord[1];
      if (
        cx < coordSys.x - DIAMOND_RADIUS ||
        cx > coordSys.x + coordSys.width + DIAMOND_RADIUS ||
        cy < coordSys.y - DIAMOND_RADIUS ||
        cy > coordSys.y + coordSys.height + DIAMOND_RADIUS
      ) {
        return null;
      }
      return {
        type: "polygon",
        shape: {
          points: [
            [cx, cy - DIAMOND_RADIUS],
            [cx + DIAMOND_RADIUS, cy],
            [cx, cy + DIAMOND_RADIUS],
            [cx - DIAMOND_RADIUS, cy],
          ],
        },
        style: {
          fill: palette.base100,
          stroke: palette.primary,
          lineWidth: 2,
        },
        emphasis: {
          style: {
            fill: palette.primary,
            stroke: palette.primary,
            lineWidth: 2,
          },
        },
      };
    }

    const width = Math.max(
      Math.max(0, endCoord[0] - startCoord[0]),
      MIN_BAR_WIDTH,
    );
    const rect = echarts.graphic.clipRectByRect(
      {
        x: startCoord[0],
        y: startCoord[1] - BAR_HEIGHT / 2,
        width,
        height: BAR_HEIGHT,
      },
      coordSys,
    );
    if (!rect) return null;

    return {
      type: "rect",
      shape: { ...rect, r: 3 },
      style: {
        fill: palette.primary,
        opacity: 0.9,
      },
      emphasis: {
        style: {
          fill: palette.primary,
          opacity: 1,
          shadowBlur: 6,
          shadowColor: palette.primaryBorder,
        },
      },
    };
  };
}

export function buildGanttOption({
  name,
  model,
  palette,
  entitiesLabel,
  formatExactTick,
  customValues,
  expandedBreaks,
}: {
  name: string;
  model: GanttChartModel;
  palette: ChartPalette;
  entitiesLabel: string;
  formatExactTick: (value: number) => string;
  customValues: number[];
  expandedBreaks: ReadonlySet<string>;
}): GanttOption {
  const needsVerticalScroll = model.events.length > MAX_VISIBLE_ROWS;
  const seriesData = model.events.map((event, rowIndex) => {
    const start = event.start_tick ?? model.minTick;
    const end = event.end_tick ?? start;
    const isPoint = event.end_tick === undefined || end <= start ? 1 : 0;
    return [rowIndex, start, end, isPoint];
  });

  return {
    animation: false,
    grid: {
      left: 168,
      right: needsVerticalScroll ? 30 : 20,
      top: 42,
      bottom: 44,
      containLabel: false,
    },
    tooltip: {
      trigger: "item",
      confine: true,
      backgroundColor: palette.base100,
      borderColor: palette.base300,
      borderWidth: 1,
      padding: [10, 12],
      textStyle: {
        color: palette.content,
        fontFamily: palette.fontSans,
        fontSize: 12,
      },
      formatter: (params) => {
        const raw = Array.isArray(params) ? params[0] : params;
        const dataIndex = raw?.dataIndex;
        if (typeof dataIndex !== "number") return "";
        const event = model.events[dataIndex];
        return event ? formatTooltipHtml(event, palette, entitiesLabel) : "";
      },
    },
    xAxis: {
      type: "value",
      position: "top",
      min: model.minTick,
      max: model.maxTick,
      scale: true,
      breaks: model.breaks.map((brk) => ({
        start: brk.start,
        end: brk.end,
        gap: "4%",
        isExpanded: expandedBreaks.has(breakKey(brk)),
      })),
      breakArea: {
        show: true,
        expandOnClick: true,
        zigzagAmplitude: 3,
        zigzagMinSpan: 4,
        zigzagMaxSpan: 10,
        itemStyle: {
          color: palette.base200,
          borderColor: palette.base300,
          borderWidth: 1,
          borderType: "dashed",
          opacity: 0.9,
        },
      },
      breakLabelLayout: {
        moveOverlap: "auto",
      },
      axisLine: {
        show: true,
        lineStyle: { color: palette.base300 },
      },
      axisTick: {
        show: true,
        length: 4,
        lineStyle: { color: palette.base300 },
        customValues,
      },
      axisLabel: {
        color: palette.contentMuted,
        fontFamily: palette.fontMono,
        fontSize: 11,
        margin: 10,
        hideOverlap: true,
        customValues,
        formatter: formatExactTick,
      },
      splitLine: {
        show: true,
        lineStyle: {
          color: palette.contentSubtle,
          type: "dashed",
        },
      },
    },
    yAxis: {
      type: "category",
      inverse: true,
      triggerEvent: true,
      data: model.events.map((event) => event.id),
      axisLine: {
        show: true,
        lineStyle: { color: palette.base300 },
      },
      axisTick: { show: false },
      splitLine: {
        show: true,
        lineStyle: { color: palette.base300, opacity: 0.5 },
      },
      splitArea: {
        show: true,
        areaStyle: {
          color: [palette.base100, palette.base200],
          opacity: 0.35,
        },
      },
      axisLabel: {
        width: 148,
        overflow: "truncate",
        ellipsis: "…",
        color: palette.content,
        fontFamily: palette.fontSans,
        fontSize: 12,
        margin: 12,
      },
    },
    dataZoom: buildDataZoom(palette, needsVerticalScroll),
    series: [
      {
        type: "custom",
        name,
        clip: true,
        renderItem: createRenderItem(palette),
        encode: { x: [1, 2], y: 0 },
        data: seriesData,
      },
    ],
  };
}
