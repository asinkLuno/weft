import {
  BookOpen,
  FileText,
  GitBranch,
  Waypoints,
  X as XIcon,
} from "lucide-react";
import { type ReactNode, useLayoutEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { WeftMark } from "@/components/weft-art";
import { EntityView } from "@/features/entity/entity-view";
import { NarrativeView } from "@/features/narrative/narrative-view";
import { StoryView } from "@/features/story/story-view";
import { type Lang, strings } from "@/i18n";
import type { WeftDoc } from "@/lib/document";
import "./workspace.css";

const TABS = ["tab1", "tab2", "tab4"] as const;
type Tab = (typeof TABS)[number];
export function Workspace({
  path,
  doc,
  lang,
  flash,
  activeError,
  onClose,
}: {
  path: string;
  doc: WeftDoc | null;
  lang: Lang;
  flash: boolean;
  activeError: unknown;
  onClose: () => void;
}) {
  const [activeTab, setActiveTab] = useState<Tab>("tab1");
  // 各页首次激活才挂载、之后常驻（实体画布要在页面可见时挂载才有尺寸可取景）
  const [visited, setVisited] = useState<ReadonlySet<Tab>>(
    () => new Set<Tab>(["tab1"]),
  );
  // 隐藏页用 display:none，滚动位置会丢：切走时记下，切回时补上
  const panes = useRef<Partial<Record<Tab, HTMLDivElement | null>>>({});
  const scrollTops = useRef<Partial<Record<Tab, number>>>({});

  function switchTab(next: Tab) {
    if (next === activeTab) return;
    const el = panes.current[activeTab];
    if (el) scrollTops.current[activeTab] = el.scrollTop;
    setActiveTab(next);
    setVisited((prev) => new Set(prev).add(next));
  }

  useLayoutEffect(() => {
    const el = panes.current[activeTab];
    const top = scrollTops.current[activeTab];
    if (el && top) el.scrollTop = top;
  }, [activeTab]);
  // 首次激活才挂载，之后常驻仅隐藏：切换不重挂载，
  // 实体画布视角和筛选状态都不丢；滚动位置由 switchTab 存取
  function pane(tab: Tab, node: ReactNode) {
    return (
      <TabsContent
        value={tab}
        className="pane"
        forceMount={visited.has(tab) || undefined}
        ref={(el) => {
          panes.current[tab] = el;
        }}
      >
        {node}
      </TabsContent>
    );
  }

  return (
    <div className="app" lang={lang === "zh" ? "zh-CN" : "en"}>
      <Tabs
        value={activeTab}
        onValueChange={(v) => switchTab(v as Tab)}
        className="tabbody"
      >
        <div className="tabbar">
          <div className="brand app-brand">
            <WeftMark />
            <span>weft</span>
          </div>
          <TabsList variant="line">
            {TABS.map((tab) => (
              <TabsTrigger key={tab} value={tab}>
                {tab === "tab1" ? (
                  <BookOpen />
                ) : tab === "tab2" ? (
                  <Waypoints />
                ) : (
                  <GitBranch />
                )}
                {strings[lang][tab]}
              </TabsTrigger>
            ))}
          </TabsList>
        </div>
        <main className="content">
          {pane("tab1", doc?.story && <StoryView doc={doc} lang={lang} />)}
          {pane("tab2", <EntityView doc={doc} lang={lang} path={path} />)}
          {pane("tab4", <NarrativeView doc={doc} lang={lang} />)}
        </main>
      </Tabs>
      <footer className={flash ? "statusbar flash" : "statusbar"}>
        <FileText className="size-3.5 shrink-0" aria-hidden="true" />
        <span className="statusbar-path" title={path}>
          {path}
        </span>
        <span className={`statusbar-state${activeError ? " has-error" : ""}`}>
          {activeError ? strings[lang].fileError : strings[lang].fileWatching}
        </span>
        <Button
          variant="ghost"
          size="icon-xs"
          className="shrink-0 text-muted-foreground"
          aria-label={strings[lang].close}
          title={strings[lang].close}
          onClick={onClose}
        >
          <XIcon />
        </Button>
      </footer>
    </div>
  );
}
