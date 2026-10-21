import dagre from "@dagrejs/dagre";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { LazyStore } from "@tauri-apps/plugin-store";
import {
  Background,
  Controls,
  type Edge,
  Handle,
  type Node,
  Position,
  ReactFlow,
  useNodesState,
} from "@xyflow/react";
import { X as XIcon } from "lucide-react";
import "@xyflow/react/dist/style.css";
import { useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { asLang, initialLang, type Lang, strings } from "./i18n";
import "./App.css";

const settings = new LazyStore("settings.json");

type DocumentPayload = {
  doc?: unknown;
  content?: string;
  error?: string;
};

// 后端已按 src-tauri/src/model.rs 校验并滤掉未声明键，这里只读序列化结果
type StoryDoc = {
  story?: {
    title?: string;
    description?: string;
    author?: string;
  };
  entity?: Record<
    string,
    {
      name?: string;
      description?: string;
      belong_to?: string[];
      base_time?: unknown;
    }
  >;
};

const YAML_FILTER = { name: "YAML", extensions: ["yaml", "yml"] };

// 四个标签页，名字在 i18n 里改
const TABS = ["tab1", "tab2", "tab3", "tab4"] as const;
type Tab = (typeof TABS)[number];

// 常驻错误提示：不自动消失，可手动关闭，下一条正常事件时清除
function showError(message: string) {
  toast.error(message, { duration: Infinity, closeButton: true });
}

function App() {
  const [path, setPath] = useState<string | null>(null);
  const [doc, setDoc] = useState<StoryDoc | null>(null);
  const [activeTab, setActiveTab] = useState<Tab>("tab1");
  const [lang, setLang] = useState<Lang>(initialLang);
  const [recents, setRecents] = useState<string[]>([]);
  const [flash, setFlash] = useState(false);
  const flashTimer = useRef<ReturnType<typeof setTimeout> | undefined>(
    undefined,
  );

  useEffect(() => {
    settings.get<unknown>("language").then((v) => {
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
    if (!path) return;
    const unlisten = listen<DocumentPayload>("document-changed", (e) => {
      const { doc: nextDoc, error } = e.payload;
      if (error) {
        showError(error === "deleted" ? strings[lang].deleted : error);
        return;
      }
      toast.dismiss();
      // 有解析结果说明文件内容更新了，短暂变色提示
      if (nextDoc !== undefined) {
        setDoc((nextDoc as StoryDoc) ?? null);
        setFlash(true);
        clearTimeout(flashTimer.current);
        flashTimer.current = setTimeout(() => setFlash(false), 400);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
      clearTimeout(flashTimer.current);
    };
  }, [path, lang]);

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
      setDoc((payload.doc as StoryDoc) ?? null);
      if (payload.error) showError(payload.error);
      remember(p);
    } catch (err) {
      showError(String(err));
      forget(p);
    }
  }

  async function openFile() {
    const p = await open({ multiple: false, filters: [YAML_FILTER] });
    if (typeof p === "string") bind(p);
  }

  if (!path) {
    return (
      <main className="welcome">
        <h1>weft</h1>
        <Button size="lg" onClick={openFile}>
          {strings[lang].open}
        </Button>
        {recents.length > 0 && (
          <>
            <h2>{strings[lang].recent}</h2>
            <ul>
              {recents.map((p) => (
                <li key={p}>
                  <Button
                    variant="link"
                    className="h-auto max-w-[560px] overflow-hidden px-1 text-ellipsis whitespace-nowrap"
                    onClick={() => bind(p)}
                  >
                    {p}
                  </Button>
                </li>
              ))}
            </ul>
          </>
        )}
      </main>
    );
  }

  return (
    <div className="app">
      <Tabs
        value={activeTab}
        onValueChange={(v) => setActiveTab(v as Tab)}
        className="tabbody"
      >
        <div className="tabbar">
          <TabsList variant="line">
            {TABS.map((tab) => (
              <TabsTrigger key={tab} value={tab}>
                {strings[lang][tab]}
              </TabsTrigger>
            ))}
          </TabsList>
        </div>
        <main className="content">
          <TabsContent value="tab1" className="pane">
            {doc?.story && <StoryView doc={doc} lang={lang} />}
          </TabsContent>
          <TabsContent value="tab2" className="pane">
            <EntityView doc={doc} lang={lang} path={path} />
          </TabsContent>
        </main>
      </Tabs>
      <footer className={flash ? "statusbar flash" : "statusbar"}>
        <span className="statusbar-path">{path}</span>
        <Button
          variant="ghost"
          size="icon-xs"
          className="text-white hover:bg-white/20 hover:text-white"
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

function StoryView({ doc, lang }: { doc: StoryDoc; lang: Lang }) {
  const t = strings[lang];
  const story = doc.story;
  if (!story) return null;
  return (
    <div className="story-body">
      <h1>{story.title}</h1>
      {story.description && <p className="story-desc">{story.description}</p>}
      {story.author && (
        <p className="story-author">
          {t.author}: {story.author}
        </p>
      )}
    </div>
  );
}

// 实体卡片节点：数据在 EntityView 里解析好，这里只管展示
type EntityT = NonNullable<StoryDoc["entity"]>[string];
type EntityData = {
  name: string;
  baseTime?: string;
  parents: Array<{ id: string; name: string }>;
  description?: string;
  belongLabel: string;
};
type EntityFlowNode = Node<EntityData, "entity">;

// 入参只要 data：既当 React Flow 节点，也能当普通卡片用
function EntityNode({ data }: { data: EntityData }) {
  return (
    <Card size="sm" className="w-[280px]">
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
      <CardHeader>
        <CardTitle>{data.name}</CardTitle>
        {data.baseTime && <CardDescription>{data.baseTime}</CardDescription>}
      </CardHeader>
      {data.parents.length > 0 && (
        <CardContent className="flex flex-wrap items-center gap-1.5">
          <span className="text-muted-foreground text-xs">
            {data.belongLabel}
          </span>
          {data.parents.map((p) => (
            <Badge key={p.id} variant="secondary">
              {p.name}
            </Badge>
          ))}
        </CardContent>
      )}
      {data.description && <CardContent>{data.description}</CardContent>}
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
  onPersist,
}: {
  initialNodes: EntityFlowNode[];
  edges: Edge[];
  // 不传则拖动不落盘（筛选视图的临时摆放）
  onPersist?: (id: string, pos: { x: number; y: number }) => void;
}) {
  const [nodes, , onNodesChange] = useNodesState(initialNodes);
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
      <Background gap={16} />
      <Controls showInteractive={false} />
    </ReactFlow>
  );
}

function EntityView({
  doc,
  lang,
  path,
}: {
  doc: StoryDoc | null;
  lang: Lang;
  path: string;
}) {
  const t = strings[lang];
  const [query, setQuery] = useState("");
  const [picked, setPicked] = useState<string | null>(null);
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
  if (entities.length === 0 || !saved) return null;

  const nameOf = (id: string) => doc?.entity?.[id]?.name ?? id;

  // id 的全部上层归属（跨多级）；后端已保证无环，先占位只是防手滑自环
  const cache = new Map<string, Set<string>>();
  const ancestorsOf = (id: string): Set<string> => {
    const hit = cache.get(id);
    if (hit) return hit;
    const set = new Set<string>();
    cache.set(id, set);
    for (const p of doc?.entity?.[id]?.belong_to ?? []) {
      set.add(p);
      for (const a of ancestorsOf(p)) set.add(a);
    }
    return set;
  };

  // 出现过的所有归属目标；已不存在的选中项视为无效，不参与过滤
  const targets = [...new Set(entities.flatMap(([, e]) => e.belong_to ?? []))];
  const activePick = picked && targets.includes(picked) ? picked : null;

  const q = query.trim().toLowerCase();
  const filtered = entities.filter(([id, e]) => {
    // 选中的实体自己也算命中，层级图里当根展示
    if (activePick && id !== activePick && !ancestorsOf(id).has(activePick))
      return false;
    if (
      q &&
      !`${e.name ?? ""} ${id} ${e.description ?? ""}`.toLowerCase().includes(q)
    )
      return false;
    return true;
  });

  const entityData = (id: string, e: EntityT): EntityData => ({
    name: nameOf(id),
    baseTime:
      e.base_time !== undefined
        ? `${t.baseTime}: ${String(e.base_time)}`
        : undefined,
    parents: (e.belong_to ?? []).map((b) => ({ id: b, name: nameOf(b) })),
    description: e.description,
    belongLabel: t.belongTo,
  });

  // 有归属筛选时按 belong_to 连边、dagre 分层；无筛选不连线、摆成网格平铺
  const visibleSet = new Set(filtered.map(([id]) => id));
  const edges: Edge[] = filtered.flatMap(([id, e]) =>
    (e.belong_to ?? [])
      .filter((p) => visibleSet.has(p))
      .map((p) => ({ id: `${p}->${id}`, source: p, target: id })),
  );
  const auto: Record<string, { x: number; y: number }> = {};
  if (activePick) {
    Object.assign(auto, dagreLayout(filtered, edges));
  } else {
    const cols = Math.ceil(Math.sqrt(filtered.length));
    filtered.forEach(([id], i) => {
      auto[id] = {
        x: (i % cols) * (ENTITY_W + 24),
        y: Math.floor(i / cols) * (ENTITY_H + 24),
      };
    });
  }
  // 筛选视图始终按 dagre 全量重排（不读、不存用户摆位）；平铺视图尊重拖过的位置
  const nodes: EntityFlowNode[] = filtered.map(([id, e]) => ({
    id,
    type: "entity",
    position: activePick ? auto[id] : (saved[id] ?? auto[id]),
    data: entityData(id, e),
  }));

  // 过滤集或筛选形态变化时重挂载画布：重新取景，筛选模式才能重排成树
  const canvasKey = `${activePick ?? ""}|${filtered.map(([id]) => id).join("|")}`;

  return (
    <div className="flex h-full flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <Input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder={t.search}
          className="max-w-64"
        />
        {targets.sort().map((tg) => (
          <Button
            key={tg}
            size="xs"
            variant={picked === tg ? "default" : "outline"}
            aria-pressed={picked === tg}
            onClick={() => setPicked((prev) => (prev === tg ? null : tg))}
          >
            {nameOf(tg)}
          </Button>
        ))}
      </div>
      {filtered.length === 0 ? (
        <p className="text-muted-foreground text-sm">{t.noMatch}</p>
      ) : (
        <div className="min-h-0 flex-1">
          <EntityCanvas
            key={canvasKey}
            initialNodes={nodes}
            edges={activePick ? edges : []}
            onPersist={
              activePick
                ? undefined
                : (id, pos) => {
                    const next = { ...saved, [id]: pos };
                    setSaved(next);
                    settings.set(layoutKey, next);
                  }
            }
          />
        </div>
      )}
    </div>
  );
}

export default App;
