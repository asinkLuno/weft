import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { LazyStore } from "@tauri-apps/plugin-store";
import { useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { Toaster } from "@/components/ui/sonner";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { asLang, initialLang, type Lang, strings } from "./i18n";
import "./App.css";

const settings = new LazyStore("settings.json");

type DocumentPayload = {
  doc?: unknown;
  content?: string;
  error?: string;
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
      const { doc, error } = e.payload;
      if (error) {
        showError(error === "deleted" ? strings[lang].deleted : error);
        return;
      }
      toast.dismiss();
      // 有解析结果说明文件内容更新了，短暂变色提示
      if (doc !== undefined) {
        setFlash(true);
        clearTimeout(flashTimer.current);
        flashTimer.current = setTimeout(() => setFlash(false), 400);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [path, lang]);

  function remember(p: string) {
    setRecents((prev) => {
      const next = [p, ...prev.filter((r) => r !== p)].slice(0, 5);
      settings.set("recentFiles", next);
      return next;
    });
  }

  // 读不到的文件（已删除等）从最近列表里移除
  function forget(p: string) {
    setRecents((prev) => {
      const next = prev.filter((r) => r !== p);
      settings.set("recentFiles", next);
      return next;
    });
  }

  async function bind(p: string) {
    try {
      const payload = await invoke<DocumentPayload>("bind_document", {
        path: p,
      });
      setPath(p);
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
        <button type="button" className="open-button" onClick={openFile}>
          {strings[lang].open}
        </button>
        {recents.length > 0 && (
          <>
            <h2>{strings[lang].recent}</h2>
            <ul>
              {recents.map((p) => (
                <li key={p}>
                  <button
                    type="button"
                    className="recent-button"
                    onClick={() => bind(p)}
                  >
                    {p}
                  </button>
                </li>
              ))}
            </ul>
          </>
        )}
        <Toaster richColors position="bottom-right" offset={40} />
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
        <main className="content" />
      </Tabs>
      <Toaster richColors position="bottom-right" offset={40} />
      <footer className={flash ? "statusbar flash" : "statusbar"}>
        <span className="statusbar-path">{path}</span>
        <span>YAML</span>
      </footer>
    </div>
  );
}

export default App;
