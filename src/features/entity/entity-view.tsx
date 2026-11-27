import "@xyflow/react/dist/style.css";
import { ChevronDown, Waypoints } from "lucide-react";
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
import { EntityDetailPanel } from "./entity-detail";
import type { Point } from "./layout";
import {
  getCachedEntityLayout,
  persistEntityPosition,
  preloadEntityLayout,
} from "./layout-store";
import { buildEntityDetail, buildEntityView } from "./model";

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
  const hasEntities = hasAnyKey(doc?.entity);

  // 通过稳定的 getPositions 回调按需读取已保存坐标：拖拽节点后再点击选中不触发 buildEntityView 重建
  const view = useMemo(
    () =>
      hasEntities && savedLayout.ready
        ? buildEntityView(
            doc,
            lang,
            filter?.dim ?? null,
            filter?.pick ?? null,
            savedLayout.getPositions(),
          )
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

  if (!hasEntities)
    return <EmptyState icon={Waypoints}>{t.entityEmpty}</EmptyState>;
  if (!view) return <EmptyState>{t.loading}</EmptyState>;

  const {
    nodes,
    edges,
    layoutMode,
    activePick,
    activeLink,
    belongSections,
    linkSections,
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
      </div>
      <div className="flex min-h-0 flex-1 overflow-hidden rounded-box border border-base-300 bg-base-200/60">
        <div className="min-h-0 min-w-0 flex-1">
          <EntityCanvas
            key={canvasKey}
            initialNodes={nodes}
            edges={edges}
            layoutMode={layoutMode}
            getSavedPositions={savedLayout.getPositions}
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
