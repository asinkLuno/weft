import dagre from "@dagrejs/dagre";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { LazyStore } from "@tauri-apps/plugin-store";
import {
  Background,
  type Edge,
  Handle,
  type Node,
  Panel,
  Position,
  ReactFlow,
  useNodesState,
  useReactFlow,
} from "@xyflow/react";
import {
  BookOpen,
  CalendarDays,
  ChevronDown,
  FileText,
  FolderOpen,
  GitBranch,
  History,
  Maximize,
  Minus,
  Plus,
  UserRound,
  Waypoints,
  X as XIcon,
} from "lucide-react";
import "@xyflow/react/dist/style.css";
import {
  Fragment,
  memo,
  type ReactNode,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { toast } from "sonner";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  HoverCard,
  HoverCardContent,
  HoverCardTrigger,
} from "@/components/ui/hover-card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { WeftMark, WeftPrint } from "@/components/weft-art";
import {
  asLang,
  type DocumentError,
  formatDocumentError,
  initialLang,
  type Lang,
  strings,
} from "./i18n";
import "./App.css";

const settings = new LazyStore("settings.json");

type DocumentPayload = {
  doc?: unknown;
  content?: string;
  error?: DocumentError;
};

// 后端已按 src-tauri/src/weft_doc.rs 校验并滤掉未声明键，这里只读序列化结果
type Phase = {
  components: number[];
  anchor: Phase | null;
};

type WeftDoc = {
  story?: {
    title?: string;
    description?: string;
    author?: string;
    date: { name: string; description?: string };
  };
  // 所选历法的时间分量数（后端解析时填充）
  calendar_components?: number;
  entity?: Record<
    string,
    {
      id: string;
      avatar?: string;
      description?: string;
      base_time?: Phase | null;
      base_time_humanizer?: string;
      // 内置 key 之外的输入字段原样透传（后端 extra_props）
      extra_props?: Record<string, unknown>;
    }
  >;
  entity_forest?: EntityForest;
  entity_link?: Record<
    string,
    Array<{ entities: [string, string]; relations: string }>
  >;
  // 事件 ID → 事件定义
  event?: Record<string, EventDefinition>;
  event_times?: Record<string, EventTiming>;
  // 叙事线名称 → 事件 ID 列表
  narrative?: Record<string, Narrative>;
};

type EntityBelongTo = {
  entity: string;
  children: EntityForest;
};

type EntityForest = EntityBelongTo[];

type Narrative = string[];

type EventDefinition = {
  start_time: Phase;
  end_time?: Phase | null;
  description?: string;
  entities?: string[];
};

type EventTiming = {
  // tick 偏移用于甘特图定位和长度，start/end 用于文本展示
  offsets?: Array<{
    entity: string;
    start_tick?: number;
    end_tick?: number;
    start?: string;
    end?: string;
  }>;
  start_tick?: number;
  end_tick?: number;
  start_display?: string;
  end_display?: string;
};

type EventItem = EventDefinition & EventTiming & { id: string; title: string };

function eventItems(doc: WeftDoc | null): EventItem[] {
  return Object.entries(doc?.event ?? {})
    .map(([id, event]) => ({
      ...event,
      ...doc?.event_times?.[id],
      id,
      title: id,
    }))
    .sort((a, b) => (a.start_tick ?? 0) - (b.start_tick ?? 0));
}

const YAML_FILTER = { name: "YAML", extensions: ["yaml", "yml"] };

// 标签页名称在 i18n 里定义
const TABS = ["tab1", "tab2", "tab4"] as const;
type Tab = (typeof TABS)[number];

// 常驻错误提示：不自动消失，可手动关闭，下一条正常事件时清除
function App() {
  const [path, setPath] = useState<string | null>(null);
  const [doc, setDoc] = useState<WeftDoc | null>(null);
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
  const [lang, setLang] = useState<Lang>(initialLang);
  const [activeError, setActiveError] = useState<unknown>(null);
  const [recents, setRecents] = useState<string[]>([]);
  const [flash, setFlash] = useState(false);
  const flashTimer = useRef<ReturnType<typeof setTimeout> | undefined>(
    undefined,
  );

  useEffect(() => {
    invoke<unknown>("get_language").then((v) => {
      const saved = asLang(v);
      if (saved) setLang(saved);
    });
    settings.get<unknown>("recentFiles").then((v) => {
      if (Array.isArray(v)) {
        setRecents(v.filter((x): x is string => typeof x === "string"));
      }
    });
  }, []);

  useEffect(() => {
    const unlisten = listen<{ language?: Lang }>("settings-changed", (e) => {
      const saved = asLang(e.payload.language);
      if (saved) setLang(saved);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    toast.dismiss();
    if (activeError !== null) {
      toast.error(
        <div className="whitespace-pre-line">
          {formatDocumentError(activeError, lang)}
        </div>,
        {
          duration: Infinity,
          closeButton: true,
        },
      );
    }
  }, [activeError, lang]);

  useEffect(() => {
    if (!path) return;
    const unlisten = listen<DocumentPayload>("document-changed", (e) => {
      const { doc: nextDoc, error } = e.payload;
      if (error) {
        setActiveError(error);
        return;
      }
      setActiveError(null);
      // 有解析结果说明文件内容更新了，短暂变色提示
      if (nextDoc !== undefined) {
        setDoc((nextDoc as WeftDoc) ?? null);
        setFlash(true);
        clearTimeout(flashTimer.current);
        flashTimer.current = setTimeout(() => setFlash(false), 400);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
      clearTimeout(flashTimer.current);
    };
  }, [path]);

  function remember(p: string) {
    const next = [p, ...recents.filter((r) => r !== p)].slice(0, 5);
    setRecents(next);
    settings.set("recentFiles", next);
  }

  // 读不到的文件（已删除等）从最近列表里移除
  function forget(p: string) {
    const next = recents.filter((r) => r !== p);
    setRecents(next);
    settings.set("recentFiles", next);
  }

  async function bind(p: string) {
    try {
      const payload = await invoke<DocumentPayload>("bind_document", {
        path: p,
      });
      setPath(p);
      setDoc((payload.doc as WeftDoc) ?? null);
      setActiveError(payload.error ?? null);
      remember(p);
    } catch (err) {
      setActiveError(err);
      forget(p);
    }
  }

  async function openFile() {
    const p = await open({ multiple: false, filters: [YAML_FILTER] });
    if (typeof p === "string") bind(p);
  }

  if (!path) {
    return (
      <main className="welcome" lang={lang === "zh" ? "zh-CN" : "en"}>
        <header className="welcome-masthead">
          <div className="brand">
            <WeftMark />
            <span>weft</span>
          </div>
          <span className="welcome-workspace">{strings[lang].workspace}</span>
        </header>
        <div className="welcome-layout">
          <section className="welcome-intro">
            <h1>{strings[lang].welcomeTitle}</h1>
            <p>{strings[lang].welcomeDescription}</p>
            <Button size="lg" onClick={openFile}>
              <FolderOpen />
              {strings[lang].open}
            </Button>
          </section>
          <div className="welcome-print" aria-hidden="true">
            <span className="welcome-wordmark">weft</span>
            <WeftPrint />
            <div className="welcome-print-legend">
              <span>
                <BookOpen />
                {strings[lang].tab1}
              </span>
              <span>
                <Waypoints />
                {strings[lang].tab2}
              </span>
              <span>
                <GitBranch />
                {strings[lang].tab4}
              </span>
            </div>
          </div>
          <Card className="recent-card">
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <History className="size-4" />
                {strings[lang].recent}
              </CardTitle>
              <CardDescription>
                {strings[lang].recentDescription}
              </CardDescription>
            </CardHeader>
            <CardContent>
              {recents.length > 0 ? (
                <ul className="recent-list">
                  {recents.map((p) => (
                    <li key={p}>
                      <Button
                        variant="ghost"
                        className="recent-file"
                        onClick={() => bind(p)}
                      >
                        <FileText className="size-5 text-primary" />
                        <span>
                          <strong>{p.split(/[\\/]/).pop()}</strong>
                          <small>{p}</small>
                        </span>
                      </Button>
                    </li>
                  ))}
                </ul>
              ) : (
                <div className="recent-empty">
                  <FileText className="size-5" aria-hidden="true" />
                  <p>{strings[lang].recentEmpty}</p>
                </div>
              )}
            </CardContent>
          </Card>
        </div>
        <p className="welcome-note">{strings[lang].fileHint}</p>
      </main>
    );
  }

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
          onClick={async () => {
            await invoke("unbind_document");
            setPath(null);
            setDoc(null);
            setActiveTab("tab1");
          }}
        >
          <XIcon />
        </Button>
      </footer>
    </div>
  );
}

function StoryView({ doc, lang }: { doc: WeftDoc; lang: Lang }) {
  const t = strings[lang];
  const story = doc.story;
  if (!story) return null;
  return (
    <div className="story-view @container mx-auto w-full max-w-5xl py-4">
      <header className="story-header mb-10 space-y-5 border-b pb-8">
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          <BookOpen className="size-4" aria-hidden="true" />
          <span>{t.tab1}</span>
        </div>
        <h1 className="break-words text-3xl font-semibold tracking-tight @min-[640px]:text-4xl">
          {story.title}
        </h1>
        {story.author && (
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <UserRound className="size-4 shrink-0" aria-hidden="true" />
            <span className="break-words">
              {t.author}：{story.author}
            </span>
          </div>
        )}
        {story.description && (
          <p className="max-w-3xl whitespace-pre-wrap break-words text-base leading-8 text-muted-foreground">
            {story.description}
          </p>
        )}
      </header>
      <div className="grid items-start gap-6 @min-[640px]:grid-cols-[minmax(0,1fr)_minmax(0,2fr)]">
        {(story.date?.name || story.date?.description) && (
          <Card className="min-w-0 gap-5">
            <CardHeader className="border-b">
              <CardTitle>
                <h2 className="flex items-center gap-2 text-sm">
                  <CalendarDays
                    className="size-4 text-muted-foreground"
                    aria-hidden="true"
                  />
                  {t.calendar}
                </h2>
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-2">
              {story.date.name && (
                <p className="break-words font-medium">{story.date.name}</p>
              )}
              {story.date.description && (
                <p className="whitespace-pre-wrap break-words text-sm leading-6 text-muted-foreground">
                  {story.date.description}
                </p>
              )}
            </CardContent>
          </Card>
        )}
      </div>
    </div>
  );
}

// 实体卡片节点：数据在 EntityView 里解析好，这里只管展示
type EntityT = NonNullable<WeftDoc["entity"]>[string];
type EntityData = {
  name: string;
  avatar?: string;
  baseTime?: string;
  parents: Array<{ id: string; name: string; preview?: EntityData }>;
  // extra_props 展开成排好序的键值对；值已转成展示文本
  extraProps: Array<[string, string]>;
  description?: string;
  belongLabel: string;
};
type EntityFlowNode = Node<EntityData, "entity">;

// 从森林收集直接归属；多重归属的实体只遍历一次子树，所有入边均保留。
function entityParents(forest: EntityForest): Map<string, Set<string>> {
  const parents = new Map<string, Set<string>>();
  const visited = new Set<string>();
  function visit(node: EntityBelongTo) {
    if (visited.has(node.entity)) return;
    visited.add(node.entity);
    for (const child of node.children) {
      const ids = parents.get(child.entity) ?? new Set<string>();
      ids.add(node.entity);
      parents.set(child.entity, ids);
      visit(child);
    }
  }
  for (const root of forest) visit(root);
  return parents;
}

// 入参只要 data：既当 React Flow 节点，也能当普通卡片用
function EntityNode({ data }: { data: EntityData }) {
  return (
    <>
      {/* 连线锚点：父级从底部出、子级从顶部入；藏掉但保留挂点，且不许用户拖出新连线 */}
      <Handle
        type="target"
        position={Position.Top}
        isConnectable={false}
        className="!border-0 !bg-transparent"
      />
      <Handle
        type="source"
        position={Position.Bottom}
        isConnectable={false}
        className="!border-0 !bg-transparent"
      />
      <EntityCard data={data} />
    </>
  );
}

function entityInitials(id: string): string {
  const parts = id
    .trim()
    .split(/[\s·._-]+/u)
    .filter(Boolean);
  if (parts.length > 1) {
    return parts
      .slice(0, 2)
      .map((part) => Array.from(part)[0])
      .join("")
      .toLocaleUpperCase();
  }
  return Array.from(parts[0] ?? "?")
    .slice(0, 2)
    .join("")
    .toLocaleUpperCase();
}

function EntityCard({ data }: { data: EntityData }) {
  return (
    <Card className="entity-card w-[280px] gap-0 overflow-hidden py-0">
      <CardHeader className="gap-3 p-4">
        <div className="flex items-start gap-3">
          <Avatar className="size-10 shrink-0 overflow-hidden">
            {data.avatar && (
              <AvatarImage
                src={data.avatar}
                alt={data.name}
                className="object-cover"
              />
            )}
            <AvatarFallback className="text-xs font-medium">
              {entityInitials(data.name)}
            </AvatarFallback>
          </Avatar>
          <CardTitle className="min-w-0 self-center break-words text-base leading-6">
            {data.name}
          </CardTitle>
        </div>
        {data.description && (
          <CardDescription className="whitespace-pre-wrap break-words leading-6">
            {data.description}
          </CardDescription>
        )}
      </CardHeader>
      {(data.baseTime ||
        data.extraProps.length > 0 ||
        data.parents.length > 0) && (
        <CardContent className="space-y-3 border-t bg-muted/30 p-4">
          {data.baseTime && (
            <div className="flex items-start gap-2 text-xs text-muted-foreground">
              <CalendarDays
                className="mt-0.5 size-3.5 shrink-0"
                aria-hidden="true"
              />
              <span className="break-words leading-5">{data.baseTime}</span>
            </div>
          )}
          {data.extraProps.map(([key, value]) => (
            <div key={key} className="flex items-baseline gap-2 text-xs">
              <span className="shrink-0 text-muted-foreground">{key}</span>
              <span className="min-w-0 break-words leading-5">{value}</span>
            </div>
          ))}
          {data.parents.length > 0 && (
            <div className="space-y-2">
              <p className="text-xs text-muted-foreground">
                {data.belongLabel}
              </p>
              <div className="flex flex-wrap gap-1.5">
                {data.parents.map((p) =>
                  p.preview ? (
                    <HoverCard key={p.id} openDelay={250}>
                      <HoverCardTrigger asChild>
                        <Button
                          variant="ghost"
                          size="sm"
                          className="nodrag max-w-full rounded-full text-left outline-none focus-visible:ring-2 focus-visible:ring-ring"
                        >
                          <Badge
                            variant="outline"
                            className="max-w-full cursor-pointer whitespace-normal break-words bg-background px-2 py-0.5 font-normal hover:bg-accent"
                          >
                            {p.name}
                          </Badge>
                        </Button>
                      </HoverCardTrigger>
                      <HoverCardContent
                        side="right"
                        align="start"
                        sideOffset={12}
                        className="nodrag nopan w-auto border-0 bg-transparent p-0 shadow-none"
                      >
                        <EntityCard data={p.preview} />
                      </HoverCardContent>
                    </HoverCard>
                  ) : (
                    <Badge
                      key={p.id}
                      variant="outline"
                      className="max-w-full whitespace-normal break-words bg-background px-2 py-0.5 font-normal"
                    >
                      {p.name}
                    </Badge>
                  ),
                )}
              </div>
            </div>
          )}
        </CardContent>
      )}
    </Card>
  );
}

const nodeTypes = { entity: EntityNode };

// 卡片实际高度随内容变化，dagre 按这个估值布点，特别高的卡片可能轻微重叠
const ENTITY_W = 280;
const ENTITY_H = 170;

// 没有用户拖放过位置的节点用 dagre 自动分层；返回各节点左上角坐标
function dagreLayout(
  entries: Array<[string, EntityT]>,
  edges: Edge[],
): Record<string, { x: number; y: number }> {
  const g = new dagre.graphlib.Graph();
  g.setGraph({ rankdir: "TB", nodesep: 48, ranksep: 64 });
  g.setDefaultEdgeLabel(() => ({}));
  for (const [id] of entries) {
    g.setNode(id, { width: ENTITY_W, height: ENTITY_H });
  }
  for (const edge of edges) g.setEdge(edge.source, edge.target);
  dagre.layout(g);
  const out: Record<string, { x: number; y: number }> = {};
  for (const [id] of entries) {
    const { x, y } = g.node(id);
    out[id] = { x: x - ENTITY_W / 2, y: y - ENTITY_H / 2 };
  }
  return out;
}

function EntityCanvas({
  initialNodes,
  edges,
  lang,
  onPersist,
}: {
  initialNodes: EntityFlowNode[];
  lang: Lang;
  edges: Edge[];
  // 不传则拖动不落盘（筛选视图的临时摆放）
  onPersist?: (id: string, pos: { x: number; y: number }) => void;
}) {
  const [nodes, setNodes, onNodesChange] = useNodesState(initialNodes);
  // useNodesState 只在挂载时读 initialNodes；文件热更新后同步卡片数据，保留用户摆位
  useEffect(() => {
    setNodes((nds) =>
      nds.map((n) => {
        const next = initialNodes.find((x) => x.id === n.id);
        return next ? { ...n, data: next.data } : n;
      }),
    );
  }, [initialNodes, setNodes]);
  return (
    <ReactFlow
      nodes={nodes}
      edges={edges}
      onNodesChange={onNodesChange}
      nodeTypes={nodeTypes}
      onNodeDragStop={
        onPersist ? (_, node) => onPersist(node.id, node.position) : undefined
      }
      fitView
      minZoom={0.1}
    >
      <Background gap={24} color="var(--border)" />
      <CanvasControls lang={lang} />
    </ReactFlow>
  );
}

function CanvasControls({ lang }: { lang: Lang }) {
  const { zoomIn, zoomOut, fitView } = useReactFlow();
  const zh = lang === "zh";
  return (
    <Panel position="bottom-left" className="canvas-controls">
      <Button
        variant="ghost"
        size="icon"
        aria-label={zh ? "放大" : "Zoom in"}
        onClick={() => zoomIn()}
      >
        <Plus />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        aria-label={zh ? "缩小" : "Zoom out"}
        onClick={() => zoomOut()}
      >
        <Minus />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        aria-label={zh ? "适应画布" : "Fit view"}
        onClick={() => fitView()}
      >
        <Maximize />
      </Button>
    </Panel>
  );
}

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
    settings
      .get<Record<string, { x: number; y: number }>>(layoutKey)
      .then((v) => setSaved(v ?? {}));
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

  const nameOf = (id: string) => doc?.entity?.[id]?.id ?? id;

  // entity_link 的引用按 id 对回实体；未知引用取 null 丢弃（后端已校验，防御手滑）
  const idOf = (r: string) => (doc?.entity && r in doc.entity ? r : null);

  const parentsById = entityParents(doc?.entity_forest ?? []);

  // id 的全部上层归属（跨多级）；后端已保证无环，先占位只是防手滑自环
  const cache = new Map<string, Set<string>>();
  const ancestorsOf = (id: string): Set<string> => {
    const hit = cache.get(id);
    if (hit) return hit;
    const set = new Set<string>();
    cache.set(id, set);
    for (const p of parentsById.get(id) ?? []) {
      set.add(p);
      for (const a of ancestorsOf(p)) set.add(a);
    }
    return set;
  };

  // 出现过的所有归属目标；已不存在的选中项视为无效，不参与过滤
  const targets = [
    ...new Set([...parentsById.values()].flatMap((ids) => [...ids])),
  ];
  const activePick =
    dim === "belong" && pick && targets.includes(pick) ? pick : null;

  // 归属目标按顶层祖先分组：霍格沃茨 → 四学院一组，可计算标签生成的维度（星座）各自成组，
  // 组内按层级深度排（根在前），组间在前端画分隔线
  const byRoot: Record<string, Array<{ v: string; label: string }>> = {};
  for (const tg of targets) {
    const root =
      [...ancestorsOf(tg)].find((a) => (parentsById.get(a)?.size ?? 0) === 0) ??
      tg;
    if (!byRoot[root]) byRoot[root] = [];
    byRoot[root].push({ v: tg, label: nameOf(tg) });
  }
  const belongSections = Object.values(byRoot).map((section) =>
    section.sort(
      (a, b) =>
        ancestorsOf(a.v).size - ancestorsOf(b.v).size ||
        a.label.localeCompare(b.label),
    ),
  );
  belongSections.sort((x, y) => x[0].label.localeCompare(y[0].label));

  // 关系组筛选同理：组在文件更新后可能消失，失效视为未选
  const groups = Object.keys(doc?.entity_link ?? {}).sort();
  const activeLink =
    dim === "link" && pick && groups.includes(pick) ? pick : null;

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
  // 选中组内出现过的实体（解析成 id）
  const linkMembers = new Set(
    activeLink
      ? (doc?.entity_link?.[activeLink] ?? [])
          .flatMap(({ entities: [a, b] }) => [idOf(a), idOf(b)])
          .filter((x): x is string => x !== null)
      : [],
  );

  const filtered = entities.filter(([id]) => {
    // 选中的实体自己也算命中，层级图里当根展示
    if (activePick && id !== activePick && !ancestorsOf(id).has(activePick))
      return false;
    if (activeLink && !linkMembers.has(id)) return false;
    return true;
  });

  // extra_props 的展示：标量直接转字符串，复合值落成 JSON 文本
  function formatExtraValue(value: unknown): string {
    return typeof value === "object" && value !== null
      ? JSON.stringify(value)
      : String(value);
  }

  const entityData = (
    id: string,
    e: EntityT,
    includePreviews = true,
  ): EntityData => ({
    name: nameOf(id),
    avatar: e.avatar ? convertFileSrc(e.avatar) : undefined,
    baseTime:
      e.base_time_humanizer !== undefined
        ? `${t.baseTime}: ${e.base_time_humanizer}`
        : undefined,
    parents: [...(parentsById.get(id) ?? [])].map((b) => ({
      id: b,
      name: nameOf(b),
      preview:
        includePreviews && doc?.entity?.[b]
          ? entityData(b, doc.entity[b], false)
          : undefined,
    })),
    extraProps: Object.entries(e.extra_props ?? {})
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([key, value]) => [key, formatExtraValue(value)]),
    description: e.description,
    belongLabel: t.belongTo,
  });

  // 有归属筛选时按森林中的归属连边、dagre 分层；无筛选不连归属线、摆成网格平铺
  const visibleSet = new Set(filtered.map(([id]) => id));
  const edges: Edge[] = filtered.flatMap(([id]) =>
    [...(parentsById.get(id) ?? [])]
      .filter((p) => visibleSet.has(p))
      .map((p) => ({ id: `${p}->${id}`, source: p, target: id })),
  );

  // 关系对只在关系筛选视图展示；端点不在当前过滤集时整条隐藏
  const linkEdges: Edge[] = Object.entries(doc?.entity_link ?? {}).flatMap(
    ([group, links]) =>
      links.flatMap(({ entities: [a, b], relations }, i) => {
        const s = idOf(a);
        const t = idOf(b);
        return s && t && s !== t && visibleSet.has(s) && visibleSet.has(t)
          ? [
              {
                id: `link:${group}:${i}`,
                source: s,
                target: t,
                label: relations,
                labelStyle: { fontSize: 12 },
              },
            ]
          : [];
      }),
  );
  const auto: Record<string, { x: number; y: number }> = {};
  if (activePick || activeLink) {
    // 归属筛选按森林分层，关系筛选按关系线分层；两个维度互斥
    Object.assign(auto, dagreLayout(filtered, activePick ? edges : linkEdges));
  } else {
    const cols = Math.ceil(Math.sqrt(filtered.length));
    filtered.forEach(([id], i) => {
      auto[id] = {
        x: (i % cols) * (ENTITY_W + 24),
        y: Math.floor(i / cols) * (ENTITY_H + 24),
      };
    });
  }
  // 筛选视图始终按 dagre 全量重排（不读、不存用户摆位）；平铺/搜索视图尊重拖过的位置
  const nodes: EntityFlowNode[] = filtered.map(([id, e]) => ({
    id,
    type: "entity",
    position: activePick || activeLink ? auto[id] : (saved[id] ?? auto[id]),
    data: entityData(id, e),
  }));

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
          edges={activePick ? edges : activeLink ? linkEdges : []}
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

const EntityView = memo(EntityViewInner);

type GanttBounds = {
  start: number;
  end: number;
  startDisplay?: string;
  endDisplay?: string;
};

type GanttScale = {
  position: (tick: number) => number;
  breaks: Array<{ start: number; end: number; label: string }>;
};

function ganttScale(events: EventItem[], origin: number): GanttScale {
  const intervals: Array<{ start: number; end: number }> = [];
  const labels = new Map<number, string>();
  for (const event of events) {
    if (event.start_tick === undefined) continue;
    const start = event.start_tick - origin;
    const end = (event.end_tick ?? event.start_tick) - origin;
    labels.set(start, event.start_display ?? "");
    labels.set(end, event.end_display ?? event.start_display ?? "");
    intervals.push({ start, end });
  }
  intervals.sort((a, b) => a.start - b.start);
  const merged: typeof intervals = [];
  for (const interval of intervals) {
    const previous = merged[merged.length - 1];
    if (previous && interval.start <= previous.end) {
      previous.end = Math.max(previous.end, interval.end);
    } else {
      merged.push({ ...interval });
    }
  }
  const activeTicks = merged.reduce(
    (sum, interval) => sum + interval.end - interval.start,
    0,
  );
  const gaps = merged
    .slice(1)
    .map((interval, index) => interval.start - merged[index].end);
  // 所有空白合计最多占活动时长的 15%；只有时间点时保留相对间距。
  const gapLimit =
    activeTicks > 0 && gaps.length > 0
      ? (activeTicks * 0.15) / gaps.length
      : Number.POSITIVE_INFINITY;
  const segments: Array<{
    start: number;
    end: number;
    offset: number;
    length: number;
  }> = [];
  const compressed: Array<{ start: number; end: number; label: string }> = [];
  let total = 0;
  for (const [index, interval] of merged.entries()) {
    if (index > 0) {
      const start = merged[index - 1].end;
      const end = interval.start;
      const length = Math.min(end - start, gapLimit);
      segments.push({ start, end, offset: total, length });
      if (length < end - start) {
        compressed.push({
          start: total,
          end: total + length,
          label: `${labels.get(start)} – ${labels.get(end)}`,
        });
      }
      total += length;
    }
    const length = interval.end - interval.start;
    segments.push({ ...interval, offset: total, length });
    total += length;
  }
  const span = Math.max(total, 1);
  return {
    position: (tick) => {
      const relative = tick - origin;
      const segment = segments.find(
        (segment) => relative >= segment.start && relative <= segment.end,
      );
      if (!segment) return relative <= 0 ? 0 : 100;
      const fraction =
        segment.end > segment.start
          ? (relative - segment.start) / (segment.end - segment.start)
          : 0;
      return ((segment.offset + fraction * segment.length) / span) * 100;
    },
    breaks: compressed.map((gap) => ({
      ...gap,
      start: (gap.start / span) * 100,
      end: (gap.end / span) * 100,
    })),
  };
}

// 所有叙事线以最早事件的 tick 为原点，共用相对时间范围。
function NarrativeGantt({
  name,
  nar,
  byId,
  bounds,
  scale,
  t,
}: {
  name: string;
  nar: Narrative;
  byId: Map<string, EventItem>;
  bounds: GanttBounds;
  scale: GanttScale;
  t: (typeof strings)[Lang];
}) {
  const events = nar.flatMap((id) => {
    const event = byId.get(id);
    return event ? [event] : [];
  });
  const originTick = bounds.start;
  const card = (event: EventItem, trigger: ReactNode) => (
    <HoverCard>
      <HoverCardTrigger asChild>{trigger}</HoverCardTrigger>
      <HoverCardContent className="w-80">
        <div className="text-sm font-medium">{event.id}</div>
        <div className="mt-0.5 font-mono text-xs text-muted-foreground">
          {event.start_display}
          {event.end_display && ` – ${event.end_display}`}
        </div>
        {event.description && (
          <p className="mt-2 text-sm whitespace-pre-wrap">
            {event.description}
          </p>
        )}
      </HoverCardContent>
    </HoverCard>
  );
  return (
    <section className="narrative-track min-w-0">
      <h2 className="mb-2 text-lg font-medium">{name}</h2>
      <div className="narrative-sheet overflow-hidden border bg-card">
        <div className="grid grid-cols-[minmax(100px,220px)_minmax(140px,1fr)] border-b bg-muted/50 text-xs text-muted-foreground">
          <div className="h-9 border-r px-3 leading-9">{t.ganttEvent}</div>
          <div className="relative flex h-9 items-end justify-between px-3 pb-1 font-mono">
            <span>{bounds.startDisplay}</span>
            <span>{bounds.endDisplay}</span>
            {scale.breaks.map((gap) => (
              <span
                key={gap.start}
                title={gap.label}
                className="absolute top-0 -translate-x-1/2 bg-card px-1 font-bold"
                style={{ left: `${(gap.start + gap.end) / 2}%` }}
              >
                {"//"}
              </span>
            ))}
          </div>
        </div>
        {events.map((event) => {
          const start = event.start_tick ?? originTick;
          const left = scale.position(start);
          const width = scale.position(event.end_tick ?? start) - left;
          return (
            <div
              key={event.id}
              className="grid min-h-14 grid-cols-[minmax(100px,220px)_minmax(140px,1fr)] border-b last:border-b-0"
            >
              <div className="min-w-0 border-r px-3 py-2">
                {card(
                  event,
                  <Button
                    variant="ghost"
                    className="block w-full truncate text-left text-sm font-medium outline-none hover:underline"
                  >
                    {event.id}
                  </Button>,
                )}
                <div className="mt-0.5 truncate text-xs text-muted-foreground">
                  {event.start_display}
                </div>
              </div>
              <div className="relative">
                {scale.breaks.map((gap) => (
                  <span
                    key={gap.start}
                    title={gap.label}
                    className="absolute inset-y-0 border-x border-dashed border-border bg-muted/40"
                    style={{
                      left: `${gap.start}%`,
                      width: `${gap.end - gap.start}%`,
                    }}
                  />
                ))}
                {card(
                  event,
                  event.end_tick === undefined ? (
                    <Button
                      aria-label={event.id}
                      className="narrative-point absolute top-1/2 min-w-3 p-0 size-3 -translate-x-1/2 -translate-y-1/2 rotate-45 rounded-[2px] border border-primary bg-primary"
                      style={{ left: `clamp(6px, ${left}%, calc(100% - 6px))` }}
                    />
                  ) : (
                    <Button
                      aria-label={event.id}
                      className="narrative-bar absolute top-1/2 min-w-3 p-0 h-6 -translate-y-1/2 border border-primary/25 bg-primary/85"
                      style={{ left: `${left}%`, width: `${width}%` }}
                    />
                  ),
                )}
              </div>
            </div>
          );
        })}
      </div>
    </section>
  );
}

function NarrativeViewInner({
  doc,
  lang,
}: {
  doc: WeftDoc | null;
  lang: Lang;
}) {
  const t = strings[lang];
  // 事件索引：id → 展示数据
  const events = eventItems(doc);
  const byId = new Map(events.map((event) => [event.id, event]));
  let first: EventItem | undefined;
  let last: EventItem | undefined;
  for (const event of events) {
    if (event.start_tick === undefined) continue;
    if (!first || event.start_tick < (first.start_tick ?? 0)) first = event;
    const end = event.end_tick ?? event.start_tick;
    if (!last || end > (last.end_tick ?? last.start_tick ?? 0)) last = event;
  }
  const bounds: GanttBounds = {
    start: first?.start_tick ?? 0,
    end: last?.end_tick ?? last?.start_tick ?? 0,
    startDisplay: first?.start_display,
    endDisplay: last?.end_display ?? last?.start_display,
  };
  const scale = ganttScale(events, bounds.start);
  const entries = Object.entries(doc?.narrative ?? {})
    .filter(([, nar]) => nar.length > 0)
    .sort(([a], [b]) => a.localeCompare(b));
  if (entries.length === 0) {
    return <div className="view-empty">{t.narrativeEmpty}</div>;
  }
  return (
    <div className="narrative-view flex h-full flex-col gap-8 overflow-auto">
      {entries.map(([name, nar]) => (
        <NarrativeGantt
          key={name}
          name={name}
          nar={nar}
          byId={byId}
          bounds={bounds}
          scale={scale}
          t={t}
        />
      ))}
    </div>
  );
}

const NarrativeView = memo(NarrativeViewInner);

export default App;
