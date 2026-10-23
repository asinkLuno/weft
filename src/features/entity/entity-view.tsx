import "@xyflow/react/dist/style.css";
import "./entity.css";
import { ChevronDown, Waypoints } from "lucide-react";
import { Fragment, memo, useEffect, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { type Lang, strings } from "@/i18n";
import type { WeftDoc } from "@/lib/document";
import { settings } from "@/lib/settings";
import { EntityCanvas } from "./entity-canvas";
import { buildEntityView } from "./model";

// 维度筛选下拉（shadcn faceted filter 同款）：outline 按钮当触发器，选中值显示为 Badge；
// sections 之间画分隔线，单组不画
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
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          variant="outline"
          size="xs"
          className="border-dashed"
          disabled={count === 0}
        >
          {label}
          {active && (
            <Badge variant="secondary" className="rounded-sm px-1 font-normal">
              {active.label}
            </Badge>
          )}
          <ChevronDown className="text-muted-foreground" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent className="max-h-72 w-48 overflow-auto">
        {sections.map((section, i) => (
          <Fragment key={section[0].v}>
            {i > 0 && <DropdownMenuSeparator />}
            {section.map(({ v, label: text }) => (
              <DropdownMenuCheckboxItem
                key={v}
                checked={active?.value === v}
                onCheckedChange={() => onChoose(v)}
              >
                {text}
              </DropdownMenuCheckboxItem>
            ))}
          </Fragment>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
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
  // 节点摆放松存 settings.json（按文件隔离），不写进用户的 YAML
  const layoutKey = `layout:${path}`;
  const [saved, setSaved] = useState<Record<
    string,
    { x: number; y: number }
  > | null>(null);

  useEffect(() => {
    setSaved(null);
    let cancelled = false;
    settings
      .get<Record<string, { x: number; y: number }>>(layoutKey)
      .then((v) => {
        if (!cancelled) setSaved(v ?? {});
      });
    return () => {
      cancelled = true;
    };
  }, [layoutKey]);

  // HashMap 无序，按 id 排序保证两次渲染顺序一致
  const entities = Object.entries(doc?.entity ?? {}).sort(([a], [b]) =>
    a.localeCompare(b),
  );
  if (entities.length === 0)
    return (
      <div className="view-empty">
        <Waypoints />
        <p>{t.entityEmpty}</p>
      </div>
    );
  if (!saved)
    return (
      <div className="view-empty">
        <p>{t.loading}</p>
      </div>
    );

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
  } = buildEntityView(doc, lang, dim, pick, saved);
  // 过滤集或筛选形态变化时重挂载画布：重新取景，筛选模式才能重排
  const canvasKey = `${activePick ?? ""}|${activeLink ?? ""}|${filtered.map(([id]) => id).join("|")}`;

  return (
    <div className="entity-view flex h-full flex-col gap-3">
      <div className="entity-toolbar flex flex-wrap items-center gap-2">
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
      <div className="entity-canvas min-h-0 flex-1">
        <EntityCanvas
          key={canvasKey}
          initialNodes={nodes}
          lang={lang}
          // 归属筛选只连归属线，关系筛选只连关系线，无筛选不连线，两个维度互斥
          edges={edges}
          onPersist={
            activePick || activeLink
              ? undefined
              : (id, pos) => {
                  const next = { ...saved, [id]: pos };
                  setSaved(next);
                  settings.set(layoutKey, next);
                }
          }
        />
      </div>
    </div>
  );
}

export const EntityView = memo(EntityViewInner);
