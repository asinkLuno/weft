import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { LazyStore } from "@tauri-apps/plugin-store";
import { useEffect, useState } from "react";
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
  const [content, setContent] = useState("");
  const [doc, setDoc] = useState<unknown>(null);
  const [error, setError] = useState("");
  const [swapped, setSwapped] = useState(false);

  useEffect(() => {
    settings.get<boolean>("swapped").then((v) => {
      if (v !== undefined && v !== null) setSwapped(v);
    });
  }, []);

  useEffect(() => {
    const unlisten = listen<boolean>("settings-changed", (e) => {
      setSwapped(e.payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    const unlisten = listen<DocumentPayload>("document-changed", (e) => {
      const { doc, content, error } = e.payload;
      if (error) {
        setError(error);
        return;
      }
      setError("");
      if (doc !== undefined) setDoc(doc);
      if (content !== undefined) setContent(content);
    });
    invoke<DocumentPayload>("bind_document", { path: STORY_PATH })
      .then(({ doc, content }) => {
        if (doc !== undefined) setDoc(doc);
        if (content !== undefined) setContent(content);
      })
      .catch((err) => setError(String(err)));
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  async function save() {
    try {
      await invoke("save_document", { content });
      setError("");
    } catch (err) {
      setError(String(err));
    }
  }

  return (
    <main
      className="panes"
      style={{ flexDirection: swapped ? "row-reverse" : "row" }}
    >
      <section className="pane">
        <header>
          <span>解析视图</span>
          {error && <span className="error">{error}</span>}
        </header>
        <pre>{JSON.stringify(doc, null, 2)}</pre>
      </section>
      <section className="pane">
        <header>
          <span>YAML 编辑器</span>
          <button type="button" onClick={save}>
            保存
          </button>
        </header>
        <textarea
          value={content}
          spellCheck={false}
          onChange={(e) => setContent(e.currentTarget.value)}
        />
      </section>
    </main>
  );
}

export default App;
