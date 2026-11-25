import { Handle, Position } from "@xyflow/react";
import { CalendarDays } from "lucide-react";
import { memo, useState } from "react";
import type { EntityData } from "./model";

const INITIALS_SPLIT_RE = /[\s·._-]+/u;

// 入参只要 data：既当 React Flow 节点，也能当普通卡片用；memo 避免拖动单个节点时重渲染其余卡片
export const EntityNode = memo(function EntityNode({
  data,
}: {
  data: EntityData;
}) {
  return (
    <>
      {/* 连线锚点：父级从底部出、子级从顶部入；藏掉但保留挂点，且不许用户拖出新连线 */}
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
      <EntityCard data={data} />
    </>
  );
});

function entityInitials(id: string): string {
  const parts = id.trim().split(INITIALS_SPLIT_RE).filter(Boolean);
  if (parts.length > 1) {
    return parts
      .slice(0, 2)
      .map((part) => Array.from(part)[0])
      .join("")
      .toLocaleUpperCase();
  }
  return Array.from(parts[0] ?? "?")
    .slice(0, 2)
    .join("")
    .toLocaleUpperCase();
}

function EntityCard({ data }: { data: EntityData }) {
  return (
    <div className="card card-sm w-[280px] border border-base-300 bg-base-100 shadow-xs">
      <div className="card-body gap-2.5">
        <div className="flex items-start gap-2.5">
          <div className="avatar avatar-placeholder shrink-0">
            <div className="w-9 rounded-full bg-neutral text-neutral-content">
              {data.avatar ? (
                <img
                  src={data.avatar}
                  alt={data.name}
                  className="object-cover"
                />
              ) : (
                <span className="text-xs font-medium">
                  {entityInitials(data.name)}
                </span>
              )}
            </div>
          </div>
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
                  <ParentBadge key={p.id} parent={p} />
                ))}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

// 预览卡只在 hover/focus 时挂载：常挂会让画布上每张卡都多出父实体那一棵 DOM
function ParentBadge({ parent }: { parent: EntityData["parents"][number] }) {
  const [open, setOpen] = useState(false);
  const preview = parent.preview;
  if (!preview) {
    return (
      <span className="badge badge-sm badge-outline max-w-full border-base-300 whitespace-normal break-words">
        {parent.name}
      </span>
    );
  }
  return (
    <fieldset
      className="dropdown dropdown-hover dropdown-right m-0 min-w-0 border-0 p-0"
      onMouseEnter={() => setOpen(true)}
      onMouseLeave={() => setOpen(false)}
      onFocus={() => setOpen(true)}
      onBlur={() => setOpen(false)}
    >
      <button
        type="button"
        tabIndex={0}
        className="nodrag badge badge-sm badge-outline h-auto max-w-full cursor-pointer border-base-300 whitespace-normal break-words transition-colors hover:border-primary hover:text-primary"
      >
        {parent.name}
      </button>
      {open && (
        <div role="dialog" className="dropdown-content z-20 w-max rounded-box">
          <EntityCard data={preview} />
        </div>
      )}
    </fieldset>
  );
}
