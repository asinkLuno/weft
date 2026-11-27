import "@xyflow/react/dist/style.css";
import { ChevronDown, Tag, Waypoints, X } from "lucide-react";
import {
  Fragment,
  memo,
  type RefObject,
  startTransition,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { EmptyState } from "@/components/empty-state";
import type { WeftDeclKind } from "@/features/editor/weft-ls-client";
import { type Lang, strings } from "@/i18n";
import type { WeftDoc } from "@/lib/document";
import { EntityCanvas } from "./entity-canvas";
import { EntityTagContext } from "./entity-card";
import { EntityDetailPanel } from "./entity-detail";
import type { Point } from "./layout";
import {
  getCachedEntityLayout,
  persistEntityPosition,
  preloadEntityLayout,
} from "./layout-store";
import {
  buildEntityDetail,
  buildEntityView,
  type EntityFilter,
  type TagOption,
} from "./model";

const EMPTY_POSITIONS: Readonly<Record<string, Point>> = {};
const getEmptyPositions = () => EMPTY_POSITIONS;
const CHEVRON_DOWN_ICON = <ChevronDown className="size-3.5" />;
const CHIP_CLOSE_ICON = <X className="size-3" aria-hidden="true" />;

function hasAnyKey(obj: Record<string, unknown> | undefined): boolean {
  if (!obj) return false;
  for (const _ in obj) return true;
  return false;
}

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
    document.addEventListener("pointerdown", handlePointerDown, {
      capture: true,
      passive: true,
    });
    document.addEventListener("keydown", handleKeyDown, true);
    return () => {
      document.removeEventListener("pointerdown", handlePointerDown, true);
      document.removeEventListener("keydown", handleKeyDown, true);
    };
  }, [ref, open]);
}

// 维度筛选下拉（faceted filter）：点击画布/外部或按 Esc 自动收起
const DimensionFilter = memo(function DimensionFilter({
  label,
  sections,
  activeValue,
  activeLabel,
  onChoose,
}: {
  label: string;
  sections: Array<Array<{ v: string; label: string }>>;
  activeValue: string | null;
  activeLabel: string | null;
  onChoose: (v: string) => void;
}) {
  const detailsRef = useRef<HTMLDetailsElement>(null);
  const [open, setOpen] = useState(false);
  useDismissDetails(detailsRef, open);

  const count = sections.reduce((n, s) => n + s.length, 0);
  const summaryClass = `btn btn-sm btn-outline ${
    activeValue ? "border-primary text-primary" : "border-base-300"
  }${count === 0 ? " btn-disabled" : ""}`;

  return (
    <details
      ref={detailsRef}
      className="dropdown"
      onToggle={(event) => setOpen(event.currentTarget.open)}
    >
      <summary className={summaryClass}>
        {label}
        {activeLabel && (
          <span className="badge badge-xs badge-primary">{activeLabel}</span>
        )}
        {CHEVRON_DOWN_ICON}
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
                  className={activeValue === v ? "menu-active" : ""}
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
});

// 标签多选交集筛选下拉：勾选不自动收起，支持一次选多个标签
const TagFilter = memo(function TagFilter({
  label,
  clearLabel,
  options,
  activeTagSet,
  onToggle,
  onClear,
}: {
  label: string;
  clearLabel: string;
  options: readonly TagOption[];
  activeTagSet: ReadonlySet<string>;
  onToggle: (tag: string) => void;
  onClear: () => void;
}) {
  const detailsRef = useRef<HTMLDetailsElement>(null);
  const [open, setOpen] = useState(false);
  useDismissDetails(detailsRef, open);

  const hasActive = activeTagSet.size > 0;
  const summaryClass = `btn btn-sm btn-outline ${
    hasActive ? "border-primary text-primary" : "border-base-300"
  }${options.length === 0 ? " btn-disabled" : ""}`;

  return (
    <details
      ref={detailsRef}
      className="dropdown"
      onToggle={(event) => setOpen(event.currentTarget.open)}
    >
      <summary className={summaryClass}>
        {label}
        {hasActive && (
          <span className="badge badge-xs badge-primary">
            {activeTagSet.size}
          </span>
        )}
        {CHEVRON_DOWN_ICON}
      </summary>
      <ul className="menu menu-sm dropdown-content z-20 mt-1 max-h-72 w-56 flex-nowrap overflow-auto rounded-box border border-base-300 bg-base-100 p-1 shadow-md">
        {options.map(({ tag, count }) => {
          const checked = activeTagSet.has(tag);
          return (
            <li key={tag}>
              <button
                type="button"
                className={`flex items-center gap-2 ${checked ? "menu-active" : ""}`}
                onClick={() => onToggle(tag)}
              >
                <input
                  type="checkbox"
                  className="checkbox checkbox-xs checkbox-primary pointer-events-none"
                  checked={checked}
                  readOnly
                  tabIndex={-1}
                />
                <span className="min-w-0 flex-1 truncate text-left">{tag}</span>
                <span className="text-xs opacity-60">{count}</span>
              </button>
            </li>
          );
        })}
        {hasActive && (
          <>
            <li aria-hidden="true" className="my-1 border-t border-base-300" />
            <li>
              <button
                type="button"
                onClick={() => {
                  onClear();
                  if (detailsRef.current) detailsRef.current.open = false;
                }}
              >
                {clearLabel}
              </button>
            </li>
          </>
        )}
      </ul>
    </details>
  );
});

function useSavedLayoutPositions(path: string) {
  const [loadedPath, setLoadedPath] = useState<string | null>(() =>
    getCachedEntityLayout(path) !== undefined ? path : null,
  );
  const ready =
    loadedPath === path || getCachedEntityLayout(path) !== undefined;

  useEffect(() => {
    if (getCachedEntityLayout(path) !== undefined) {
      setLoadedPath(path);
      return;
    }
    let cancelled = false;
    void preloadEntityLayout(path).then(() => {
      if (!cancelled) setLoadedPath(path);
    });
    return () => {
      cancelled = true;
    };
  }, [path]);

  const getPositions = useCallback(
    (): Readonly<Record<string, Point>> => getCachedEntityLayout(path) ?? {},
    [path],
  );

  const persist = useCallback(
    (id: string, pos: Point) => {
      persistEntityPosition(path, id, pos);
    },
    [path],
  );

  return {
    ready,
    getPositions,
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
  const [filter, setFilter] = useState<EntityFilter | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [focusTarget, setFocusTarget] = useState<{
    id: string;
    seq: number;
  } | null>(null);
  const focusSeqRef = useRef(0);

  const savedLayout = useSavedLayoutPositions(path);
  const hasEntities = hasAnyKey(doc?.entity);

  // 通过稳定的 getPositions 回调按需读取已保存坐标：拖拽节点后再点击选中不触发 buildEntityView 重建
  const view = useMemo(
    () =>
      hasEntities && savedLayout.ready
        ? buildEntityView(doc, lang, filter, savedLayout.getPositions())
        : null,
    [
      doc,
      lang,
      filter,
      hasEntities,
      savedLayout.ready,
      savedLayout.getPositions,
    ],
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

  const chooseBelong = useCallback((pick: string) => {
    startTransition(() => {
      setFilter((prev) =>
        prev?.dim === "belong" && prev.pick === pick
          ? null
          : { dim: "belong", pick },
      );
    });
  }, []);

  const chooseLink = useCallback((pick: string) => {
    startTransition(() => {
      setFilter((prev) =>
        prev?.dim === "link" && prev.pick === pick
          ? null
          : { dim: "link", pick },
      );
    });
  }, []);

  const toggleTag = useCallback((tag: string) => {
    startTransition(() => {
      setFilter((prev) => {
        const current = prev?.dim === "tags" ? prev.picks : [];
        const next = current.includes(tag)
          ? current.filter((item) => item !== tag)
          : [...current, tag];
        return next.length > 0 ? { dim: "tags", picks: next } : null;
      });
    });
  }, []);

  const clearTags = useCallback(() => {
    startTransition(() => {
      setFilter((prev) => (prev?.dim === "tags" ? null : prev));
    });
  }, []);

  const activeTagsKey = view?.activeTags.join("\0") ?? "";
  const activeTagSet = useMemo(
    () => new Set(activeTagsKey ? activeTagsKey.split("\0") : []),
    [activeTagsKey],
  );
  const tagContextValue = useMemo(
    () => ({ activeTags: activeTagSet, onToggleTag: toggleTag }),
    [activeTagSet, toggleTag],
  );

  if (!hasEntities)
    return <EmptyState icon={Waypoints}>{t.entityEmpty}</EmptyState>;
  if (!view) return <EmptyState>{t.loading}</EmptyState>;

  const {
    nodes,
    edges,
    layoutMode,
    useSavedPositions,
    activePick,
    activeLink,
    activeTags,
    belongSections,
    linkSections,
    tagOptions,
    nameOf,
  } = view;
  const canvasKey = `${path}|${activePick ?? ""}|${activeLink ?? ""}`;

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <DimensionFilter
          label={t.belongTo}
          sections={belongSections}
          activeValue={activePick}
          activeLabel={activePick ? nameOf(activePick) : null}
          onChoose={chooseBelong}
        />
        <DimensionFilter
          label={t.linkGroup}
          sections={linkSections}
          activeValue={activeLink}
          activeLabel={activeLink}
          onChoose={chooseLink}
        />
        <TagFilter
          label={t.tags}
          clearLabel={t.clearFilter}
          options={tagOptions}
          activeTagSet={activeTagSet}
          onToggle={toggleTag}
          onClear={clearTags}
        />
        {activeTags.map((tag) => (
          <button
            key={tag}
            type="button"
            className="badge badge-sm badge-primary cursor-pointer gap-1"
            onClick={() => toggleTag(tag)}
          >
            <span>{tag}</span>
            {CHIP_CLOSE_ICON}
          </button>
        ))}
        {activeTags.length > 1 && (
          <button
            type="button"
            className="btn btn-ghost btn-xs"
            onClick={clearTags}
          >
            {t.clearFilter}
          </button>
        )}
      </div>
      <div className="flex min-h-0 flex-1 overflow-hidden rounded-box border border-base-300 bg-base-200/60">
        <EntityTagContext.Provider value={tagContextValue}>
          <div className="min-h-0 min-w-0 flex-1">
            {activeTags.length > 0 && nodes.length === 0 ? (
              <EmptyState
                icon={Tag}
                action={
                  <button
                    type="button"
                    className="btn btn-sm btn-outline border-base-300"
                    onClick={clearTags}
                  >
                    {t.clearFilter}
                  </button>
                }
              >
                {t.entityTagEmpty}
              </EmptyState>
            ) : (
              <EntityCanvas
                key={canvasKey}
                initialNodes={nodes}
                edges={edges}
                layoutMode={layoutMode}
                viewKey={activeTagsKey}
                getSavedPositions={
                  useSavedPositions
                    ? savedLayout.getPositions
                    : getEmptyPositions
                }
                selectedId={selectedDetail?.id ?? null}
                focusTarget={focusTarget}
                lang={lang}
                onSelect={setSelectedId}
                onJumpToEntity={handleJumpToEntity}
                onPersist={useSavedPositions ? savedLayout.persist : undefined}
              />
            )}
          </div>
        </EntityTagContext.Provider>
        {selectedDetail && (
          <EntityDetailPanel
            detail={selectedDetail}
            lang={lang}
            activeTags={activeTagSet}
            onClose={handleCloseDetail}
            onSelectEntity={handleJumpToEntity}
            onToggleTag={toggleTag}
            onRevealInEditor={onRevealInEditor}
          />
        )}
      </div>
    </div>
  );
}

export const EntityView = memo(EntityViewInner);
