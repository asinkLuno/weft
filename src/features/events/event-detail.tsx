import {
  ChevronLeft,
  ChevronRight,
  GitBranch,
  List,
  Users,
} from "lucide-react";
import {
  DetailBadgeList,
  DetailPanel,
  DetailSection,
  DetailTimeRow,
  EntityAvatar,
} from "@/components/detail-panel";
import type { WeftDeclKind } from "@/features/editor/weft-completion";
import { type Lang, strings } from "@/i18n";
import type { EventDetail } from "./model";

function EventNavButtons({
  prevId,
  nextId,
  prevLabel,
  nextLabel,
  onSelectEvent,
}: {
  prevId?: string;
  nextId?: string;
  prevLabel: string;
  nextLabel: string;
  onSelectEvent: (eventId: string) => void;
}) {
  if (!prevId && !nextId) return null;
  return (
    <div className="join">
      <button
        type="button"
        className="btn btn-outline btn-xs btn-square join-item border-base-300"
        disabled={!prevId}
        aria-label={prevLabel}
        title={prevId ?? prevLabel}
        onClick={() => prevId && onSelectEvent(prevId)}
      >
        <ChevronLeft className="size-3.5" />
      </button>
      <button
        type="button"
        className="btn btn-outline btn-xs btn-square join-item border-base-300"
        disabled={!nextId}
        aria-label={nextLabel}
        title={nextId ?? nextLabel}
        onClick={() => nextId && onSelectEvent(nextId)}
      >
        <ChevronRight className="size-3.5" />
      </button>
    </div>
  );
}

export function EventDetailPanel({
  detail,
  lang,
  onClose,
  onSelectEvent,
  onRevealInEditor,
}: {
  detail: EventDetail;
  lang: Lang;
  onClose: () => void;
  onSelectEvent: (eventId: string) => void;
  onRevealInEditor: (kind: WeftDeclKind, name: string) => void;
}) {
  const t = strings[lang];
  const isEmpty =
    !detail.time &&
    !detail.description &&
    detail.entities.length === 0 &&
    detail.lists.length === 0;

  return (
    <DetailPanel
      ariaLabel={t.eventDetails}
      title={detail.id}
      leading={
        <GitBranch
          className="mt-1 size-4 shrink-0 text-primary"
          aria-hidden="true"
        />
      }
      closeLabel={t.closeDetail}
      locateLabel={t.locateInEditor}
      emptyLabel={t.entityNoDetails}
      isEmpty={isEmpty}
      className="rounded-box border border-base-300"
      extraActions={
        <EventNavButtons
          prevId={detail.prevId}
          nextId={detail.nextId}
          prevLabel={t.prevEvent}
          nextLabel={t.nextEvent}
          onSelectEvent={onSelectEvent}
        />
      }
      onClose={onClose}
      onLocate={() => onRevealInEditor("event", detail.id)}
    >
      {detail.time && <DetailTimeRow value={detail.time} />}
      {detail.description && (
        <p className="whitespace-pre-wrap break-words text-sm leading-6 text-base-content/85">
          {detail.description}
        </p>
      )}

      {detail.entities.length > 0 && (
        <DetailSection title={t.ganttEntities} icon={Users}>
          <ul className="flex flex-col gap-1.5">
            {detail.entities.map((entity) => (
              <li key={entity.id}>
                <button
                  type="button"
                  className="flex w-full cursor-pointer items-start gap-2 rounded-box border border-base-300/80 bg-base-200/40 p-2 text-left transition-colors hover:border-primary/50 hover:bg-base-200/70"
                  title={t.locateInEditor}
                  onClick={() => onRevealInEditor("entity", entity.id)}
                >
                  <EntityAvatar
                    name={entity.name}
                    avatar={entity.avatar}
                    size="sm"
                  />
                  <div className="min-w-0 flex-1">
                    <div className="break-words font-semibold text-base-content">
                      {entity.name}
                    </div>
                    {entity.description && (
                      <p className="mt-0.5 line-clamp-2 whitespace-pre-wrap break-words leading-4 text-base-content/65">
                        {entity.description}
                      </p>
                    )}
                  </div>
                </button>
              </li>
            ))}
          </ul>
        </DetailSection>
      )}

      {detail.lists.length > 0 && (
        <DetailSection title={t.eventLists} icon={List}>
          <DetailBadgeList
            items={detail.lists.map((name) => ({ id: name, label: name }))}
            title={t.locateInEditor}
            onClick={(listName) => onRevealInEditor("events", listName)}
          />
        </DetailSection>
      )}
    </DetailPanel>
  );
}
