import { memo, type ReactNode, useState } from "react";
import type { Lang, strings } from "@/i18n";
import type { Narrative } from "@/lib/document";
import type { EventItem, GanttBounds, GanttScale } from "./model";

function EventHoverContent({ event }: { event: EventItem }) {
  return (
    <>
      <div className="text-sm font-semibold">{event.id}</div>
      <div className="mt-0.5 font-mono text-xs text-base-content/60">
        {event.start_display}
        {event.end_display && ` – ${event.end_display}`}
      </div>
      {event.description && (
        <p className="mt-2 whitespace-pre-wrap text-xs leading-5 text-base-content/80">
          {event.description}
        </p>
      )}
    </>
  );
}

// 悬浮卡：daisyUI 的 dropdown-hover，触发器与内容都带 tabindex 以支持键盘
function EventHoverCard({
  event,
  label,
  placement = "top",
  className,
  style,
  children,
}: {
  event: EventItem;
  label?: string;
  placement?: "top" | "bottom";
  className?: string;
  style?: React.CSSProperties;
  children: ReactNode;
}) {
  // 内容只在 hover/focus 时挂载：每个事件两个悬浮卡，常挂会翻倍 DOM
  const [open, setOpen] = useState(false);
  return (
    <fieldset
      className={`dropdown dropdown-hover ${
        placement === "bottom" ? "dropdown-bottom" : "dropdown-top"
      } m-0 min-w-0 border-0 p-0 ${className ?? ""}`}
      style={style}
      onMouseEnter={() => setOpen(true)}
      onMouseLeave={() => setOpen(false)}
      onFocus={() => setOpen(true)}
      onBlur={() => setOpen(false)}
    >
      <button
        type="button"
        tabIndex={0}
        aria-label={label}
        className="block w-full cursor-pointer text-left"
      >
        {children}
      </button>
      {open && (
        <div
          role="dialog"
          className="dropdown-content z-20 w-72 rounded-box border border-base-300 bg-base-100 p-3 shadow-md"
        >
          <EventHoverContent event={event} />
        </div>
      )}
    </fieldset>
  );
}

// 所有叙事线以最早事件的 tick 为原点，共用相对时间范围。
export const NarrativeGantt = memo(function NarrativeGantt({
  name,
  nar,
  byId,
  bounds,
  scale,
  t,
}: {
  name: string;
  nar: Narrative;
  byId: Map<string, EventItem>;
  bounds: GanttBounds;
  scale: GanttScale;
  t: (typeof strings)[Lang];
}) {
  const events = nar.flatMap((id) => {
    const event = byId.get(id);
    return event ? [event] : [];
  });
  const originTick = bounds.start;
  return (
    <section className="min-w-0">
      <h2 className="mb-2 text-base font-semibold">{name}</h2>
      <div className="rounded-box border border-base-300 bg-base-100">
        <div className="grid grid-cols-[minmax(100px,220px)_minmax(140px,1fr)] rounded-t-box border-b border-base-300 bg-base-200 text-xs text-base-content/60">
          <div className="h-8 border-r border-base-300 px-3 leading-8">
            {t.ganttEvent}
          </div>
          <div className="relative flex h-8 items-end justify-between px-3 pb-1 font-mono">
            <span>{bounds.startDisplay}</span>
            <span>{bounds.endDisplay}</span>
            {scale.breaks.map((gap) => (
              <span
                key={gap.start}
                title={gap.label}
                className="absolute top-0 -translate-x-1/2 bg-base-200 px-1 font-bold"
                style={{ left: `${(gap.start + gap.end) / 2}%` }}
              >
                {"//"}
              </span>
            ))}
          </div>
        </div>
        {events.map((event, index) => {
          const start = event.start_tick ?? originTick;
          const left = scale.position(start);
          const width = scale.position(event.end_tick ?? start) - left;
          const placement = index === 0 ? "bottom" : "top";
          return (
            <div
              key={event.id}
              className="grid min-h-12 grid-cols-[minmax(100px,220px)_minmax(140px,1fr)] border-b border-base-300 last:border-b-0"
            >
              <div className="min-w-0 border-r border-base-300 px-3 py-2">
                <EventHoverCard
                  event={event}
                  placement={placement}
                  className="dropdown-start w-full"
                >
                  <span className="block truncate text-sm font-medium transition-colors hover:text-primary">
                    {event.id}
                  </span>
                </EventHoverCard>
                <div className="mt-0.5 truncate font-mono text-xs text-base-content/50">
                  {event.start_display}
                </div>
              </div>
              <div className="relative">
                {scale.breaks.map((gap) => (
                  <span
                    key={gap.start}
                    title={gap.label}
                    className="absolute inset-y-0 border-x border-dashed border-base-300 bg-base-200/70"
                    style={{
                      left: `${gap.start}%`,
                      width: `${gap.end - gap.start}%`,
                    }}
                  />
                ))}
                {event.end_tick === undefined ? (
                  <EventHoverCard
                    event={event}
                    label={event.id}
                    placement={placement}
                    className="absolute top-1/2 -translate-1/2"
                    style={{
                      left: `clamp(6px, ${left}%, calc(100% - 6px))`,
                    }}
                  >
                    <span className="block size-3 rotate-45 rounded-[2px] border-2 border-primary bg-base-100 transition-colors hover:bg-primary" />
                  </EventHoverCard>
                ) : (
                  <EventHoverCard
                    event={event}
                    label={event.id}
                    placement={placement}
                    className="absolute top-1/2 -translate-y-1/2"
                    style={{ left: `${left}%`, width: `${width}%` }}
                  >
                    <span className="block h-4 min-w-3 rounded-xs bg-primary transition-opacity hover:opacity-80" />
                  </EventHoverCard>
                )}
              </div>
            </div>
          );
        })}
      </div>
    </section>
  );
});
