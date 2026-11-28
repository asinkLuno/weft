import { Handle, Position } from "@xyflow/react";
import { CalendarDays } from "lucide-react";
import { memo } from "react";
import { EntityAvatar } from "@/components/detail-panel";
import type { EntityData } from "./model";

const CALENDAR_ICON = (
  <CalendarDays className="mt-0.5 size-3.5 shrink-0" aria-hidden="true" />
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
}: {
  data: EntityData;
  selected?: boolean;
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
          <div className="min-w-0 flex-1 self-center">
            <h3 className="break-words text-sm leading-5 font-semibold">
              {data.name}
            </h3>
            {data.aliases.length > 0 && (
              <div className="mt-1 flex flex-wrap gap-1">
                {data.aliases.map((alias) => (
                  <span
                    key={alias}
                    className="badge badge-xs badge-ghost max-w-full truncate text-[10px]"
                  >
                    = {alias}
                  </span>
                ))}
              </div>
            )}
          </div>
        </div>
        {data.description && (
          <p className="line-clamp-3 whitespace-pre-wrap break-words text-xs leading-5 text-base-content/70">
            {data.description}
          </p>
        )}
      </div>
      {(data.baseTime || data.extraProps.length > 0) && (
        <div className="flex flex-col gap-2 border-t border-base-300 px-4 py-3">
          {data.baseTime && (
            <div className="flex items-start gap-1.5 text-xs text-base-content/60">
              {CALENDAR_ICON}
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
        </div>
      )}
    </div>
  );
}
