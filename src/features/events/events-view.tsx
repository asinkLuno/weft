import { GitBranch } from "lucide-react";
import { memo, useMemo } from "react";
import { EmptyState } from "@/components/empty-state";
import { type Lang, strings } from "@/i18n";
import type { WeftDoc } from "@/lib/document";
import { EventsGantt } from "./events-gantt";
import { buildEventsView } from "./model";

function EventsViewInner({ doc, lang }: { doc: WeftDoc | null; lang: Lang }) {
  const t = strings[lang];
  const { byId, bounds, scale, entries } = useMemo(
    () => buildEventsView(doc),
    [doc],
  );
  if (entries.length === 0) {
    return <EmptyState icon={GitBranch}>{t.eventsEmpty}</EmptyState>;
  }
  return (
    <div className="flex h-full flex-col gap-6 overflow-auto">
      {entries.map(([name, ids]) => (
        <EventsGantt
          key={name}
          name={name}
          ids={ids}
          byId={byId}
          bounds={bounds}
          scale={scale}
          t={t}
        />
      ))}
    </div>
  );
}

export const EventsView = memo(EventsViewInner);
