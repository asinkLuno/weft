import type { ReactNode } from "react";
import { Button } from "@/components/ui/button";
import {
  HoverCard,
  HoverCardContent,
  HoverCardTrigger,
} from "@/components/ui/hover-card";
import type { Lang, strings } from "@/i18n";
import type { Narrative } from "@/lib/document";
import type { EventItem, GanttBounds, GanttScale } from "./model";
// 所有叙事线以最早事件的 tick 为原点，共用相对时间范围。
export function NarrativeGantt({
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
  const card = (event: EventItem, trigger: ReactNode) => (
    <HoverCard>
      <HoverCardTrigger asChild>{trigger}</HoverCardTrigger>
      <HoverCardContent className="w-80">
        <div className="text-sm font-medium">{event.id}</div>
        <div className="mt-0.5 font-mono text-xs text-muted-foreground">
          {event.start_display}
          {event.end_display && ` – ${event.end_display}`}
        </div>
        {event.description && (
          <p className="mt-2 text-sm whitespace-pre-wrap">
            {event.description}
          </p>
        )}
      </HoverCardContent>
    </HoverCard>
  );
  return (
    <section className="narrative-track min-w-0">
      <h2 className="mb-2 text-lg font-medium">{name}</h2>
      <div className="narrative-sheet overflow-hidden border bg-card">
        <div className="grid grid-cols-[minmax(100px,220px)_minmax(140px,1fr)] border-b bg-muted/50 text-xs text-muted-foreground">
          <div className="h-9 border-r px-3 leading-9">{t.ganttEvent}</div>
          <div className="relative flex h-9 items-end justify-between px-3 pb-1 font-mono">
            <span>{bounds.startDisplay}</span>
            <span>{bounds.endDisplay}</span>
            {scale.breaks.map((gap) => (
              <span
                key={gap.start}
                title={gap.label}
                className="absolute top-0 -translate-x-1/2 bg-card px-1 font-bold"
                style={{ left: `${(gap.start + gap.end) / 2}%` }}
              >
                {"//"}
              </span>
            ))}
          </div>
        </div>
        {events.map((event) => {
          const start = event.start_tick ?? originTick;
          const left = scale.position(start);
          const width = scale.position(event.end_tick ?? start) - left;
          return (
            <div
              key={event.id}
              className="grid min-h-14 grid-cols-[minmax(100px,220px)_minmax(140px,1fr)] border-b last:border-b-0"
            >
              <div className="min-w-0 border-r px-3 py-2">
                {card(
                  event,
                  <Button
                    variant="ghost"
                    className="block w-full truncate text-left text-sm font-medium outline-none hover:underline"
                  >
                    {event.id}
                  </Button>,
                )}
                <div className="mt-0.5 truncate text-xs text-muted-foreground">
                  {event.start_display}
                </div>
              </div>
              <div className="relative">
                {scale.breaks.map((gap) => (
                  <span
                    key={gap.start}
                    title={gap.label}
                    className="absolute inset-y-0 border-x border-dashed border-border bg-muted/40"
                    style={{
                      left: `${gap.start}%`,
                      width: `${gap.end - gap.start}%`,
                    }}
                  />
                ))}
                {card(
                  event,
                  event.end_tick === undefined ? (
                    <Button
                      aria-label={event.id}
                      className="narrative-point absolute top-1/2 min-w-3 p-0 size-3 -translate-x-1/2 -translate-y-1/2 rotate-45 rounded-[2px] border border-primary bg-primary"
                      style={{ left: `clamp(6px, ${left}%, calc(100% - 6px))` }}
                    />
                  ) : (
                    <Button
                      aria-label={event.id}
                      className="narrative-bar absolute top-1/2 min-w-3 p-0 h-6 -translate-y-1/2 border border-primary/25 bg-primary/85"
                      style={{ left: `${left}%`, width: `${width}%` }}
                    />
                  ),
                )}
              </div>
            </div>
          );
        })}
      </div>
    </section>
  );
}
