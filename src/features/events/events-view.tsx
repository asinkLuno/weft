import { GitBranch } from "lucide-react";
import { lazy, memo, Suspense, useCallback, useMemo, useState } from "react";
import { EmptyState } from "@/components/empty-state";
import type { WeftDeclKind } from "@/features/editor/weft-completion";
import { type Lang, strings } from "@/i18n";
import type { WeftDoc } from "@/lib/document";
import { EventDetailPanel } from "./event-detail";
import { buildEventDetail, buildEventsView } from "./model";

const EventsGantt = lazy(() =>
  import("./events-gantt").then((m) => ({ default: m.EventsGantt })),
);

function EventsViewInner({
  doc,
  lang,
  onRevealInEditor,
}: {
  doc: WeftDoc | null;
  lang: Lang;
  onRevealInEditor: (kind: WeftDeclKind, name: string) => void;
}) {
  const t = strings[lang];
  const [selection, setSelection] = useState<{
    eventId: string;
    listName: string;
  } | null>(null);

  const eventsView = useMemo(() => buildEventsView(doc), [doc]);
  const { byId, entries, calendarId } = eventsView;

  const selectedDetail = useMemo(
    () =>
      selection
        ? buildEventDetail(
            doc,
            selection.eventId,
            selection.listName,
            eventsView,
          )
        : null,
    [doc, selection, eventsView],
  );

  const handleSelectEvent = useCallback((eventId: string, listName: string) => {
    setSelection({ eventId, listName });
  }, []);

  const handleNavigateEvent = useCallback((eventId: string) => {
    setSelection((prev) =>
      prev ? { eventId, listName: prev.listName } : null,
    );
  }, []);

  const handleCloseDetail = useCallback(() => {
    setSelection(null);
  }, []);

  if (entries.length === 0) {
    return <EmptyState icon={GitBranch}>{t.eventsEmpty}</EmptyState>;
  }
  const calendarKey = `${calendarId}:${lang}`;

  return (
    <div className="flex h-full min-h-0 gap-4 overflow-hidden">
      <div className="flex min-h-0 min-w-0 flex-1 flex-col gap-6 overflow-auto">
        <Suspense fallback={null}>
          {entries.map(([name, ids]) => (
            <EventsGantt
              key={`${name}:${calendarId}`}
              name={name}
              ids={ids}
              byId={byId}
              calendarKey={calendarKey}
              t={t}
              onSelectEvent={handleSelectEvent}
            />
          ))}
        </Suspense>
      </div>
      {selectedDetail && (
        <EventDetailPanel
          detail={selectedDetail}
          lang={lang}
          onClose={handleCloseDetail}
          onSelectEvent={handleNavigateEvent}
          onRevealInEditor={onRevealInEditor}
        />
      )}
    </div>
  );
}

export const EventsView = memo(EventsViewInner);
