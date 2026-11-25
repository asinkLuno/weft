import "@xyflow/react/dist/style.css";
import { ChevronDown, Waypoints } from "lucide-react";
import {
  Fragment,
  memo,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { EmptyState } from "@/components/empty-state";
import { type Lang, strings } from "@/i18n";
import type { WeftDoc } from "@/lib/document";
import { settings } from "@/lib/settings";
import { EntityCanvas } from "./entity-canvas";
import { buildEntityView } from "./model";

// 维度筛选下拉（faceted filter）：outline 按钮当触发器，选中值显示为 badge
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
  const count = sections.reduce((n, s) => n + s.length, 0);
  return (
    <details className="dropdown">
      <summary
        className={`btn btn-outline btn-xs${count === 0 ? " btn-disabled" : ""}`}
      >
        {label}
        {active && <span className="badge badge-sm">{active.label}</span>}
        <ChevronDown className="size-3.5" />
      </summary>
      <ul className="menu dropdown-content z-10 max-h-72 w-48 overflow-auto rounded-box bg-base-100 p-1 shadow">
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
                  onClick={(event) => {
                    onChoose(v);
                    event.currentTarget
                      .closest("details")
                      ?.removeAttribute("open");
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

// memo：flash 等无关状态变化时不重跑排序/布局/建节点
function EntityViewInner({
  doc,
  lang,
  path,
}: {
  doc: WeftDoc | null;
  lang: Lang;
  path: string;
}) {
  const t = strings[lang];
  // 两级筛选：先选维度（归属/关系），再选值；dim 为 null 即无筛选
  const [dim, setDim] = useState<"belong" | "link" | null>(null);
  const [pick, setPick] = useState<string | null>(null);
  // 节点摆放松存 settings.json（按文件隔离），不写进用户的 .weft；
  // 拖动落位写入 ref + store，不触发整个实体图重建（React Flow 内部已持有最新位置）。
  const layoutKey = `layout:${path}`;
  const savedPositionsRef = useRef<Record<string, { x: number; y: number }>>(
    {},
  );
  const [loadedLayoutKey, setLoadedLayoutKey] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    settings
      .get<Record<string, { x: number; y: number }>>(layoutKey)
      .then((v) => {
        if (cancelled) return;
        savedPositionsRef.current = v ?? {};
        setLoadedLayoutKey(layoutKey);
      });
    return () => {
      cancelled = true;
    };
  }, [layoutKey]);

  const savedReady = loadedLayoutKey === layoutKey;
  const hasEntities = Object.keys(doc?.entity ?? {}).length > 0;

  const view = useMemo(
    () =>
      hasEntities && savedReady
        ? buildEntityView(doc, lang, dim, pick, savedPositionsRef.current)
        : null,
    [doc, lang, dim, pick, hasEntities, savedReady],
  );

  const handlePersist = useCallback(
    (id: string, pos: { x: number; y: number }) => {
      const next = { ...savedPositionsRef.current, [id]: pos };
      savedPositionsRef.current = next;
      void settings.set(layoutKey, next);
    },
    [layoutKey],
  );

  if (!hasEntities)
    return <EmptyState icon={Waypoints}>{t.entityEmpty}</EmptyState>;
  if (!view) return <EmptyState>{t.loading}</EmptyState>;

  // 再选已选中的值 = 取消筛选
  function choose(d: "belong" | "link", v: string) {
    if (dim === d && pick === v) {
      setDim(null);
      setPick(null);
    } else {
      setDim(d);
      setPick(v);
    }
  }
  const {
    nodes,
    edges,
    activePick,
    activeLink,
    belongSections,
    groups,
    filtered,
    nameOf,
  } = view;
  // 过滤集或筛选形态变化时重挂载画布：重新取景，筛选模式才能重排
  const canvasKey = `${activePick ?? ""}|${activeLink ?? ""}|${filtered.map(([id]) => id).join("|")}`;

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
      <div className="min-h-0 flex-1">
        <EntityCanvas
          key={canvasKey}
          initialNodes={nodes}
          lang={lang}
          // 归属筛选只连归属线，关系筛选只连关系线，无筛选不连线，两个维度互斥
          edges={edges}
          onPersist={activePick || activeLink ? undefined : handlePersist}
        />
      </div>
    </div>
  );
}

export const EntityView = memo(EntityViewInner);
