import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { LazyStore } from "@tauri-apps/plugin-store";
import { useEffect, useRef, useState } from "react";
import { asLang, initialLang, type Lang, strings } from "./i18n";
import "./App.css";

const settings = new LazyStore("settings.json");

type DocumentPayload = {
  doc?: unknown;
  content?: string;
  error?: string;
};

const YAML_FILTER = { name: "YAML", extensions: ["yaml", "yml"] };

function ErrorToast({
  message,
  onClose,
}: {
  message: string;
  onClose: () => void;
}) {
  return (
    <div className="error-toast">
      <span>{message}</span>
      <button type="button" onClick={onClose} aria-label="close">
        ×
      </button>
    </div>
  );
}

function App() {
  const [path, setPath] = useState<string | null>(null);
  const [doc, setDoc] = useState<unknown>(null);
  const [error, setError] = useState("");
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
        setError(error);
        return;
      }
      setError("");
      if (doc !== undefined) {
        setDoc(doc);
        // 短暂变色提示内容已更新
        setFlash(true);
        clearTimeout(flashTimer.current);
        flashTimer.current = setTimeout(() => setFlash(false), 400);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [path]);

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
      setDoc(payload.doc ?? null);
      setError(payload.error ?? "");
      remember(p);
    } catch (err) {
      setError(String(err));
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
        {error && <ErrorToast message={error} onClose={() => setError("")} />}
      </main>
    );
  }

  return (
    <div className="app">
      <main className="panes">
        <section className="pane">
          <header>
            <span>{strings[lang].parseView}</span>
          </header>
          <pre className={flash ? "flash" : ""}>
            {JSON.stringify(doc, null, 2)}
          </pre>
        </section>
      </main>
      {error && <ErrorToast message={error} onClose={() => setError("")} />}
      <footer className="statusbar">
        <span className="statusbar-path">{path}</span>
        <span>YAML</span>
      </footer>
    </div>
  );
}

export default App;
