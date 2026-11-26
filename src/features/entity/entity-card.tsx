import { Handle, Position } from "@xyflow/react";
import { CalendarDays } from "lucide-react";
import {
  createContext,
  memo,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
} from "react";
import { createPortal } from "react-dom";
import { EntityAvatar } from "@/components/detail-panel";
import type { EntityData } from "./model";

const PREVIEW_WIDTH = 280;
const PREVIEW_GAP = 10;
const VIEWPORT_MARGIN = 12;

export const EntitySelectContext = createContext<((id: string) => void) | null>(
  null,
);

// 入参只要 data 与 selected：memo 避免拖动单个节点时重渲染其余卡片
export const EntityNode = memo(function EntityNode({
  data,
  selected,
}: {
  data: EntityData;
  selected?: boolean;
}) {
  return (
    <>
      {/* 归属层级边锚点：父级从底部出、子级从顶部入；藏掉但保留挂点，且不许用户拖出新连线 */}
      <Handle
        type="target"
        position={Position.Top}
        isConnectable={false}
        className="!border-0 !bg-transparent"
      />
      <Handle
        type="source"
        position={Position.Bottom}
        isConnectable={false}
        className="!border-0 !bg-transparent"
      />
      <EntityCard data={data} selected={selected} />
    </>
  );
});

export function EntityCard({
  data,
  selected = false,
  interactive = true,
}: {
  data: EntityData;
  selected?: boolean;
  interactive?: boolean;
}) {
  return (
    <div
      className={`card card-sm w-[280px] border bg-base-100 shadow-xs transition-colors ${
        selected
          ? "border-primary ring-2 ring-primary/25"
          : "border-base-300 hover:border-base-content/30"
      }`}
    >
      <div className="card-body gap-2.5">
        <div className="flex items-start gap-2.5">
          <EntityAvatar name={data.name} avatar={data.avatar} size="md" />
          <h3 className="min-w-0 self-center break-words text-sm leading-5 font-semibold">
            {data.name}
          </h3>
        </div>
        {data.description && (
          <p className="line-clamp-3 whitespace-pre-wrap break-words text-xs leading-5 text-base-content/70">
            {data.description}
          </p>
        )}
      </div>
      {(data.baseTime ||
        data.extraProps.length > 0 ||
        data.parents.length > 0) && (
        <div className="flex flex-col gap-2 border-t border-base-300 px-4 py-3">
          {data.baseTime && (
            <div className="flex items-start gap-1.5 text-xs text-base-content/60">
              <CalendarDays
                className="mt-0.5 size-3.5 shrink-0"
                aria-hidden="true"
              />
              <span className="break-words font-mono leading-5">
                {data.baseTime}
              </span>
            </div>
          )}
          {data.extraProps.map(([key, value]) => (
            <div key={key} className="flex items-baseline gap-2 text-xs">
              <span className="shrink-0 text-base-content/60">{key}</span>
              <span className="min-w-0 break-words leading-5">{value}</span>
            </div>
          ))}
          {data.parents.length > 0 && (
            <div className="flex flex-col gap-1.5">
              <p className="text-xs text-base-content/60">{data.belongLabel}</p>
              <div className="flex flex-wrap gap-1">
                {data.parents.map((p) => (
                  <ParentBadge
                    key={p.id}
                    parent={p}
                    interactive={interactive}
                  />
                ))}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

type PreviewCoords = { left: number; top: number };

function computePreviewCoords(anchor: DOMRect): PreviewCoords {
  const spaceRight = window.innerWidth - anchor.right - PREVIEW_GAP;
  const preferRight = spaceRight >= PREVIEW_WIDTH + VIEWPORT_MARGIN;
  const left = preferRight
    ? anchor.right + PREVIEW_GAP
    : Math.max(VIEWPORT_MARGIN, anchor.left - PREVIEW_GAP - PREVIEW_WIDTH);
  // 留出底部边距，避免预览卡贴底被视口截断
  const maxTop = Math.max(VIEWPORT_MARGIN, window.innerHeight - 260);
  const top = Math.min(Math.max(VIEWPORT_MARGIN, anchor.top), maxTop);
  return { left, top };
}

// 预览卡通过 Portal 挂到 document.body，脱离 React Flow 节点的堆叠上下文，不会被相邻卡片盖住
function ParentBadge({
  parent,
  interactive,
}: {
  parent: EntityData["parents"][number];
  interactive: boolean;
}) {
  const onSelectEntity = useContext(EntitySelectContext);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const [coords, setCoords] = useState<PreviewCoords | null>(null);
  const preview = interactive ? parent.preview : undefined;

  const showPreview = useCallback(() => {
    const rect = triggerRef.current?.getBoundingClientRect();
    if (!rect) return;
    setCoords(computePreviewCoords(rect));
  }, []);

  const hidePreview = useCallback(() => {
    setCoords(null);
  }, []);

  const isOpen = coords !== null;

  // 画布缩放或滚动时自动收起预览，防止浮层停在旧坐标
  useEffect(() => {
    if (!isOpen) return;
    const dismiss = () => setCoords(null);
    window.addEventListener("wheel", dismiss, { passive: true });
    window.addEventListener("pointerdown", dismiss, {
      capture: true,
      passive: true,
    });
    return () => {
      window.removeEventListener("wheel", dismiss);
      window.removeEventListener("pointerdown", dismiss, { capture: true });
    };
  }, [isOpen]);

  if (!preview) {
    return (
      <span className="badge badge-sm badge-outline h-auto max-w-full border-base-300 whitespace-normal break-words">
        {parent.name}
      </span>
    );
  }

  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        className="nodrag badge badge-sm badge-outline h-auto max-w-full cursor-pointer border-base-300 whitespace-normal break-words transition-colors hover:border-primary hover:text-primary"
        onMouseEnter={showPreview}
        onMouseLeave={hidePreview}
        onFocus={showPreview}
        onBlur={hidePreview}
        onClick={(event) => {
          event.stopPropagation();
          setCoords(null);
          onSelectEntity?.(parent.id);
        }}
      >
        {parent.name}
      </button>
      {coords &&
        createPortal(
          <div
            role="tooltip"
            className="pointer-events-none fixed z-50 rounded-box shadow-lg"
            style={{ left: coords.left, top: coords.top }}
          >
            <EntityCard data={preview} interactive={false} />
          </div>,
          document.body,
        )}
    </>
  );
}
