import {
  ArrowRight,
  GitBranch,
  Link2,
  Tag,
  Users,
  Waypoints,
} from "lucide-react";
import { memo, useCallback } from "react";
import {
  DetailBadgeList,
  DetailPanel,
  DetailSection,
  DetailTimeRow,
  EntityAvatar,
} from "@/components/detail-panel";
import type { WeftDeclKind } from "@/features/editor/weft-ls-client";
import { type Lang, strings } from "@/i18n";
import type { EntityDetail, EntityEventEntry, EntityLinkEntry } from "./model";

const LINK_ARROW_ICON = (
  <ArrowRight
    className="size-3.5 shrink-0 text-primary/70"
    aria-hidden="true"
  />
);

function EntityLinksSection({
  title,
  locateLabel,
  links,
  onSelectEntity,
  onRevealInEditor,
}: {
  title: string;
  locateLabel: string;
  links: readonly EntityLinkEntry[];
  onSelectEntity: (id: string) => void;
  onRevealInEditor: (kind: WeftDeclKind, name: string) => void;
}) {
  return (
    <DetailSection title={title} icon={Link2}>
      <ul className="flex flex-col gap-1.5">
        {links.map((item) => (
          <li
            key={item.key}
            className="flex flex-wrap items-center gap-1.5 rounded-box border border-base-300/80 bg-base-200/40 px-2.5 py-1.5"
          >
            <button
              type="button"
              className="badge badge-xs badge-ghost cursor-pointer hover:text-primary"
              title={locateLabel}
              onClick={() => onRevealInEditor("link", item.group)}
            >
              {item.group}
            </button>
            {item.direction === "incoming" ? (
              <>
                <button
                  type="button"
                  className="link link-hover min-w-0 break-words font-medium"
                  onClick={() => onSelectEntity(item.peerId)}
                >
                  {item.peerName}
                </button>
                <span className="font-medium text-primary">
                  {item.relation}
                </span>
                {LINK_ARROW_ICON}
              </>
            ) : (
              <>
                <span className="font-medium text-primary">
                  {item.relation}
                </span>
                {item.direction === "outgoing" && LINK_ARROW_ICON}
                <button
                  type="button"
                  className="link link-hover min-w-0 break-words font-medium"
                  onClick={() => onSelectEntity(item.peerId)}
                >
                  {item.peerName}
                </button>
              </>
            )}
          </li>
        ))}
      </ul>
    </DetailSection>
  );
}

function EntityEventsSection({
  title,
  events,
  onRevealInEditor,
}: {
  title: string;
  events: readonly EntityEventEntry[];
  onRevealInEditor: (kind: WeftDeclKind, name: string) => void;
}) {
  return (
    <DetailSection title={title} icon={GitBranch}>
      <ul className="flex flex-col gap-2">
        {events.map((ev) => (
          <li
            key={ev.id}
            className="[content-visibility:auto] [contain-intrinsic-size:auto_64px]"
          >
            <button
              type="button"
              className="flex w-full cursor-pointer flex-col gap-1 rounded-box border border-base-300/80 bg-base-200/40 p-2.5 text-left transition-colors hover:border-primary/50 hover:bg-base-200/70"
              onClick={() => onRevealInEditor("event", ev.id)}
            >
              <span className="break-words font-semibold text-base-content">
                {ev.id}
              </span>
              {ev.time && (
                <span className="break-words font-mono text-[11px] text-base-content/60">
                  {ev.time}
                </span>
              )}
              {ev.description && (
                <span className="whitespace-pre-wrap break-words leading-5 text-base-content/75">
                  {ev.description}
                </span>
              )}
            </button>
          </li>
        ))}
      </ul>
    </DetailSection>
  );
}

export const EntityDetailPanel = memo(function EntityDetailPanel({
  detail,
  lang,
  activeTags,
  onClose,
  onSelectEntity,
  onToggleTag,
  onRevealInEditor,
}: {
  detail: EntityDetail;
  lang: Lang;
  activeTags: ReadonlySet<string>;
  onClose: () => void;
  onSelectEntity: (id: string) => void;
  onToggleTag: (tag: string) => void;
  onRevealInEditor: (kind: WeftDeclKind, name: string) => void;
}) {
  const t = strings[lang];
  const handleLocate = useCallback(
    () => onRevealInEditor("entity", detail.id),
    [detail.id, onRevealInEditor],
  );
  const isEmpty =
    !detail.description &&
    !detail.baseTime &&
    detail.tags.length === 0 &&
    detail.extraProps.length === 0 &&
    detail.parents.length === 0 &&
    detail.children.length === 0 &&
    detail.links.length === 0 &&
    detail.events.length === 0;

  return (
    <DetailPanel
      ariaLabel={t.entityDetails}
      title={detail.name}
      leading={
        <EntityAvatar name={detail.name} avatar={detail.avatar} size="lg" />
      }
      closeLabel={t.closeDetail}
      locateLabel={t.locateInEditor}
      emptyLabel={t.entityNoDetails}
      isEmpty={isEmpty}
      className="border-l border-base-300"
      onClose={onClose}
      onLocate={handleLocate}
    >
      {detail.description && (
        <p className="whitespace-pre-wrap break-words text-sm leading-6 text-base-content/85">
          {detail.description}
        </p>
      )}
      {detail.baseTime && <DetailTimeRow value={detail.baseTime} />}

      {detail.tags.length > 0 && (
        <DetailSection title={t.tags} icon={Tag}>
          <div className="flex flex-wrap gap-1.5">
            {detail.tags.map((tag) => (
              <button
                key={tag}
                type="button"
                className={`badge badge-sm h-auto cursor-pointer whitespace-normal break-words transition-colors ${
                  activeTags.has(tag)
                    ? "badge-primary"
                    : "badge-soft badge-primary"
                }`}
                onClick={() => onToggleTag(tag)}
              >
                {tag}
              </button>
            ))}
          </div>
        </DetailSection>
      )}

      {detail.extraProps.length > 0 && (
        <DetailSection title={t.entityProps}>
          <dl className="flex flex-col gap-1.5">
            {detail.extraProps.map(([key, value]) => (
              <div key={key} className="flex items-baseline gap-2">
                <dt className="shrink-0 text-base-content/60">{key}</dt>
                <dd className="min-w-0 break-words leading-5">{value}</dd>
              </div>
            ))}
          </dl>
        </DetailSection>
      )}

      {detail.parents.length > 0 && (
        <DetailSection title={t.belongTo} icon={Waypoints}>
          <DetailBadgeList items={detail.parents} onClick={onSelectEntity} />
        </DetailSection>
      )}

      {detail.children.length > 0 && (
        <DetailSection title={t.entityChildren} icon={Users}>
          <DetailBadgeList items={detail.children} onClick={onSelectEntity} />
        </DetailSection>
      )}

      {detail.links.length > 0 && (
        <EntityLinksSection
          title={t.entityLinks}
          locateLabel={t.locateInEditor}
          links={detail.links}
          onSelectEntity={onSelectEntity}
          onRevealInEditor={onRevealInEditor}
        />
      )}

      {detail.events.length > 0 && (
        <EntityEventsSection
          title={t.entityEvents}
          events={detail.events}
          onRevealInEditor={onRevealInEditor}
        />
      )}
    </DetailPanel>
  );
});
