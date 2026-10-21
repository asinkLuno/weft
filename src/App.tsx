import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { LazyStore } from "@tauri-apps/plugin-store";
import { useEffect, useState } from "react";
import { asLang, initialLang, type Lang, strings } from "./i18n";
import "./App.css";

// 验证期写死的绑定路径（tauri dev 的 cwd 是 src-tauri）；正式版换成文件对话框
const STORY_PATH = "../examples/story.yaml";

const settings = new LazyStore("settings.json");

type DocumentPayload = {
  doc?: unknown;
  content?: string;
  error?: string;
};

function App() {
  const [doc, setDoc] = useState<unknown>(null);
  const [error, setError] = useState("");
  const [lang, setLang] = useState<Lang>(initialLang);

  useEffect(() => {
    settings.get<unknown>("language").then((v) => {
      const saved = asLang(v);
      if (saved) setLang(saved);
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
    const unlisten = listen<DocumentPayload>("document-changed", (e) => {
      const { doc, error } = e.payload;
      if (error) {
        setError(error);
        return;
      }
      setError("");
      if (doc !== undefined) setDoc(doc);
    });
    invoke<DocumentPayload>("bind_document", { path: STORY_PATH })
      .then(({ doc }) => {
        if (doc !== undefined) setDoc(doc);
      })
      .catch((err) => setError(String(err)));
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return (
    <main className="panes">
      <section className="pane">
        <header>
          <span>{strings[lang].parseView}</span>
          {error && <span className="error">{error}</span>}
        </header>
        <pre>{JSON.stringify(doc, null, 2)}</pre>
      </section>
    </main>
  );
}

export default App;
