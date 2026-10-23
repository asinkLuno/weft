import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState } from "react";
import type { DocumentError } from "@/i18n";
import type { WeftDoc } from "@/lib/document";

type DocumentPayload = {
  doc: WeftDoc | null;
  content: string | null;
  error: DocumentError | null;
};

const YAML_FILTER = { name: "YAML", extensions: ["yaml", "yml"] };

// 所有文档命令与事件集中在这里；绑定和解绑串行执行。
export function useDocument() {
  const [path, setPath] = useState<string | null>(null);
  const [doc, setDoc] = useState<WeftDoc | null>(null);
  const [activeError, setActiveError] = useState<unknown>(null);
  const [flash, setFlash] = useState(false);
  const boundPath = useRef<string | null>(null);
  const busy = useRef(false);
  const pendingChange = useRef<DocumentPayload | null>(null);
  const mounted = useRef(false);
  const listener = useRef<Promise<UnlistenFn> | null>(null);
  const flashTimer = useRef<ReturnType<typeof setTimeout> | undefined>(
    undefined,
  );

  useEffect(() => {
    mounted.current = true;
    let cancelled = false;
    const subscription = listen<DocumentPayload>(
      "document-changed",
      ({ payload }) => {
        if (cancelled) return;
        if (busy.current) {
          pendingChange.current = payload;
          return;
        }
        if (!boundPath.current) return;
        if (payload.error) {
          setActiveError(payload.error);
          return;
        }
        setActiveError(null);
        setDoc(payload.doc);
        setFlash(true);
        clearTimeout(flashTimer.current);
        flashTimer.current = setTimeout(() => setFlash(false), 400);
      },
    );
    listener.current = subscription;
    void subscription.catch((error: unknown) => {
      if (!cancelled) setActiveError(error);
    });
    return () => {
      cancelled = true;
      mounted.current = false;
      clearTimeout(flashTimer.current);
      void subscription.then((unlisten) => unlisten()).catch(console.error);
    };
  }, []);

  function takePendingChange(): DocumentPayload | null {
    const payload = pendingChange.current;
    pendingChange.current = null;
    return payload;
  }

  async function bind(
    nextPath: string,
  ): Promise<"bound" | "failed" | "ignored"> {
    if (busy.current) return "ignored";
    busy.current = true;
    pendingChange.current = null;
    try {
      // 先完成订阅，再绑定文件，避免初次打开时漏掉 watcher 更新。
      await listener.current;
      const payload = await invoke<DocumentPayload>("bind_document", {
        path: nextPath,
      });
      if (!mounted.current) return "ignored";
      boundPath.current = nextPath;
      setPath(nextPath);
      // 绑定期间也可能收到文件变更，优先采用较新的有效快照。
      const changed = takePendingChange();
      setDoc(changed && !changed.error ? changed.doc : payload.doc);
      setActiveError(changed ? changed.error : payload.error);
      clearTimeout(flashTimer.current);
      setFlash(false);
      return "bound";
    } catch (error) {
      if (mounted.current) setActiveError(error);
      return "failed";
    } finally {
      pendingChange.current = null;
      busy.current = false;
    }
  }

  async function openFile(): Promise<string | null> {
    try {
      const selected = await open({ multiple: false, filters: [YAML_FILTER] });
      return typeof selected === "string" ? selected : null;
    } catch (error) {
      if (mounted.current) setActiveError(error);
      return null;
    }
  }

  async function close() {
    if (busy.current) return;
    busy.current = true;
    try {
      await invoke("unbind_document");
      if (!mounted.current) return;
      boundPath.current = null;
      setPath(null);
      setDoc(null);
      setActiveError(null);
      clearTimeout(flashTimer.current);
      setFlash(false);
    } catch (error) {
      if (mounted.current) setActiveError(error);
    } finally {
      busy.current = false;
    }
  }

  return { path, doc, activeError, flash, bind, openFile, close };
}
