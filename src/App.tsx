import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { LazyStore } from "@tauri-apps/plugin-store";
import { useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
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
      belong_to?: string;
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
            <EntityView doc={doc} lang={lang} />
          </TabsContent>
        </main>
      </Tabs>
      <footer className={flash ? "statusbar flash" : "statusbar"}>
        <span className="statusbar-path">{path}</span>
        <span>YAML</span>
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

function EntityView({ doc, lang }: { doc: StoryDoc | null; lang: Lang }) {
  const t = strings[lang];
  // HashMap 无序，按 id 排序保证两次渲染顺序一致
  const entities = Object.entries(doc?.entity ?? {}).sort(([a], [b]) =>
    a.localeCompare(b),
  );
  if (entities.length === 0) return null;
  return (
    <div className="flex flex-col gap-3">
      {entities.map(([id, e]) => (
        <Card key={id} size="sm">
          <CardHeader>
            <CardTitle>{e.name ?? id}</CardTitle>
            <CardDescription>
              {[
                e.belong_to && `${t.belongTo}: ${e.belong_to}`,
                e.base_time !== undefined &&
                  `${t.baseTime}: ${String(e.base_time)}`,
              ]
                .filter(Boolean)
                .join(" · ")}
            </CardDescription>
          </CardHeader>
          {e.description && <CardContent>{e.description}</CardContent>}
        </Card>
      ))}
    </div>
  );
}

export default App;
