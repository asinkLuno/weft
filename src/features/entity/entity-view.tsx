import "@xyflow/react/dist/style.css";
import { ChevronDown, Waypoints } from "lucide-react";
import {
  Fragment,
  memo,
  type RefObject,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { EmptyState } from "@/components/empty-state";
import type { WeftDeclKind } from "@/features/editor/weft-completion";
import { type Lang, strings } from "@/i18n";
import type { WeftDoc } from "@/lib/document";
import { settings } from "@/lib/settings";
import { EntityCanvas } from "./entity-canvas";
import { EntityDetailPanel } from "./entity-detail";
import type { Point } from "./layout";
import { buildEntityDetail, buildEntityView } from "./model";

function useDismissDetails(
  ref: RefObject<HTMLDetailsElement | null>,
  open: boolean,
) {
  useEffect(() => {
    if (!open) return;
    function handlePointerDown(event: PointerEvent) {
      const el = ref.current;
      if (el && event.target instanceof Node && !el.contains(event.target)) {
        el.open = false;
      }
    }
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape" && ref.current?.open) {
        ref.current.open = false;
        event.stopImmediatePropagation();
      }
    }
    document.addEventListener("pointerdown", handlePointerDown, true);
    document.addEventListener("keydown", handleKeyDown, true);
    return () => {
      document.removeEventListener("pointerdown", handlePointerDown, true);
      document.removeEventListener("keydown", handleKeyDown, true);
    };
  }, [ref, open]);
}

// 维度筛选下拉（faceted filter）：点击画布/外部或按 Esc 自动收起
function DimensionFilter({
  label,
  sections,
  active,
  onChoose,
}: {
  label: string;
  sections: Array<Array<{ v: string; label: string }>>;
  active: { value: string; label: string } | null;
  onChoose: (v: string) => void;
}) {
  const detailsRef = useRef<HTMLDetailsElement>(null);
  const [open, setOpen] = useState(false);
  useDismissDetails(detailsRef, open);

  const count = sections.reduce((n, s) => n + s.length, 0);
  const summaryClass = `btn btn-sm btn-outline ${
    active ? "border-primary text-primary" : "border-base-300"
  }${count === 0 ? " btn-disabled" : ""}`;

  return (
    <details
      ref={detailsRef}
      className="dropdown"
      onToggle={(event) => setOpen(event.currentTarget.open)}
    >
      <summary className={summaryClass}>
        {label}
        {active && (
          <span className="badge badge-xs badge-primary">{active.label}</span>
        )}
        <ChevronDown className="size-3.5" />
      </summary>
      <ul className="menu menu-sm dropdown-content z-20 mt-1 max-h-72 w-48 overflow-auto rounded-box border border-base-300 bg-base-100 p-1 shadow-md">
        {sections.map((section, i) => (
          <Fragment key={section[0]?.v ?? String(i)}>
            {i > 0 && (
              <li
                aria-hidden="true"
                className="my-1 border-t border-base-300"
              />
            )}
            {section.map(({ v, label: text }) => (
              <li key={v}>
                <button
                  type="button"
                  className={active?.value === v ? "menu-active" : ""}
                  onClick={() => {
                    onChoose(v);
                    if (detailsRef.current) detailsRef.current.open = false;
                  }}
                >
                  {text}
                </button>
              </li>
            ))}
          </Fragment>
        ))}
      </ul>
    </details>
  );
}

function useSavedLayoutPositions(path: string) {
  const layoutKey = `layout:${path}`;
  const savedRef = useRef<Record<string, Point>>({});
  const [loadedKey, setLoadedKey] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    settings.get<Record<string, Point>>(layoutKey).then((v) => {
      if (cancelled) return;
      savedRef.current = v ?? {};
      setLoadedKey(layoutKey);
    });
    return () => {
      cancelled = true;
    };
  }, [layoutKey]);

  const persist = useCallback(
    (id: string, pos: Point) => {
      const next = { ...savedRef.current, [id]: pos };
      savedRef.current = next;
      void settings.set(layoutKey, next);
    },
    [layoutKey],
  );

  return {
    ready: loadedKey === layoutKey,
    positions: savedRef.current,
    persist,
  };
}

// memo：flash 等无关状态变化时不重跑排序/布局/建节点
function EntityViewInner({
  doc,
  lang,
  path,
  onRevealInEditor,
}: {
  doc: WeftDoc | null;
  lang: Lang;
  path: string;
  onRevealInEditor: (kind: WeftDeclKind, name: string) => void;
}) {
  const t = strings[lang];
  const [filter, setFilter] = useState<{
    dim: "belong" | "link";
    pick: string;
  } | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [focusTarget, setFocusTarget] = useState<{
    id: string;
    seq: number;
  } | null>(null);
  const focusSeqRef = useRef(0);

  const savedLayout = useSavedLayoutPositions(path);
  const hasEntities = Object.keys(doc?.entity ?? {}).length > 0;

  const view = useMemo(
    () =>
      hasEntities && savedLayout.ready
        ? buildEntityView(
            doc,
            lang,
            filter?.dim ?? null,
            filter?.pick ?? null,
            savedLayout.positions,
          )
        : null,
    [doc, lang, filter, hasEntities, savedLayout.ready, savedLayout.positions],
  );

  const selectedDetail = useMemo(
    () => (selectedId ? buildEntityDetail(doc, lang, selectedId) : null),
    [doc, lang, selectedId],
  );

  const handleJumpToEntity = useCallback((id: string) => {
    setSelectedId(id);
    focusSeqRef.current += 1;
    setFocusTarget({ id, seq: focusSeqRef.current });
  }, []);

  const handleCloseDetail = useCallback(() => setSelectedId(null), []);

  if (!hasEntities)
    return <EmptyState icon={Waypoints}>{t.entityEmpty}</EmptyState>;
  if (!view) return <EmptyState>{t.loading}</EmptyState>;

  function choose(dim: "belong" | "link", pick: string) {
    setFilter((prev) =>
      prev?.dim === dim && prev.pick === pick ? null : { dim, pick },
    );
  }

  const {
    nodes,
    edges,
    layoutMode,
    activePick,
    activeLink,
    belongSections,
    groups,
    nameOf,
  } = view;
  const canvasKey = `${path}|${activePick ?? ""}|${activeLink ?? ""}`;

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <DimensionFilter
          label={t.belongTo}
          sections={belongSections}
          active={
            activePick ? { value: activePick, label: nameOf(activePick) } : null
          }
          onChoose={(v) => choose("belong", v)}
        />
        <DimensionFilter
          label={t.linkGroup}
          sections={[groups.map((g) => ({ v: g, label: g }))]}
          active={activeLink ? { value: activeLink, label: activeLink } : null}
          onChoose={(v) => choose("link", v)}
        />
      </div>
      <div className="flex min-h-0 flex-1 overflow-hidden rounded-box border border-base-300 bg-base-200/60">
        <div className="min-h-0 min-w-0 flex-1">
          <EntityCanvas
            key={canvasKey}
            initialNodes={nodes}
            edges={edges}
            layoutMode={layoutMode}
            savedPositions={savedLayout.positions}
            selectedId={selectedDetail?.id ?? null}
            focusTarget={focusTarget}
            lang={lang}
            onSelect={setSelectedId}
            onJumpToEntity={handleJumpToEntity}
            onPersist={layoutMode === "grid" ? savedLayout.persist : undefined}
          />
        </div>
        {selectedDetail && (
          <EntityDetailPanel
            detail={selectedDetail}
            lang={lang}
            onClose={handleCloseDetail}
            onSelectEntity={handleJumpToEntity}
            onRevealInEditor={onRevealInEditor}
          />
        )}
      </div>
    </div>
  );
}

export const EntityView = memo(EntityViewInner);
