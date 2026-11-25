import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useRef, useState } from "react";
import type { DocumentError } from "@/i18n";
import type { WeftDoc } from "@/lib/document";

type DocumentPayload = {
  generation: number;
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
  const boundGeneration = useRef<number | null>(null);
  const busy = useRef(false);
  const pendingChange = useRef<DocumentPayload | null>(null);
  const mounted = useRef(false);
  const sessionToken = useRef(0);
  const unlistenRef = useRef<UnlistenFn | null>(null);
  const listenerReady = useRef<Promise<void> | null>(null);
  const flashTimer = useRef<ReturnType<typeof setTimeout> | undefined>(
    undefined,
  );

  const startListener = useCallback((resetBackend: boolean): Promise<void> => {
    const token = ++sessionToken.current;
    const prevUnlisten = unlistenRef.current;
    unlistenRef.current = null;
    prevUnlisten?.();

    const setup = (async () => {
      if (resetBackend) {
        // 清理页面刷新/HMR 前后端可能残留的旧 watcher
        await invoke("unbind_document").catch(() => {});
      }
      if (token !== sessionToken.current) return;
      const unlisten = await listen<DocumentPayload>(
        "document-changed",
        ({ payload }) => {
          if (token !== sessionToken.current) return;
          if (busy.current) {
            pendingChange.current = payload;
            return;
          }
          if (
            boundGeneration.current === null ||
            payload.generation !== boundGeneration.current
          ) {
            return;
          }
          setDoc(payload.doc);
          setActiveError(payload.error);
          if (payload.error) return;
          setFlash(true);
          clearTimeout(flashTimer.current);
          flashTimer.current = setTimeout(() => setFlash(false), 400);
        },
      );
      if (token !== sessionToken.current) {
        unlisten();
        return;
      }
      unlistenRef.current = unlisten;
    })();

    listenerReady.current = setup;
    void setup.catch((error: unknown) => {
      if (token === sessionToken.current) {
        listenerReady.current = null;
        setActiveError(error);
      }
    });
    return setup;
  }, []);

  useEffect(() => {
    mounted.current = true;
    void startListener(true);
    return () => {
      mounted.current = false;
      sessionToken.current += 1;
      listenerReady.current = null;
      clearTimeout(flashTimer.current);
      const unlisten = unlistenRef.current;
      unlistenRef.current = null;
      unlisten?.();
      if (boundGeneration.current !== null || busy.current) {
        boundGeneration.current = null;
        void invoke("unbind_document").catch(() => {});
      }
    };
  }, [startListener]);

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
      // 先完成订阅（若首次订阅失败则自动重试），再绑定文件，避免初次打开时漏掉 watcher 更新。
      const ready = listenerReady.current ?? startListener(false);
      await ready;
      const payload = await invoke<DocumentPayload>("bind_document", {
        path: nextPath,
      });
      if (!mounted.current) {
        void invoke("unbind_document").catch(() => {});
        return "ignored";
      }
      // 绑定期间若收到同一会话代数的新变更，优先采用较新快照；旧文档迟到事件按代数丢弃。
      const changed = takePendingChange();
      const effective =
        changed && changed.generation === payload.generation
          ? changed
          : payload;
      boundGeneration.current = payload.generation;
      setPath(nextPath);
      setDoc(effective.doc);
      setActiveError(effective.error);
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
    pendingChange.current = null;
    try {
      await invoke("unbind_document");
      if (!mounted.current) return;
      boundGeneration.current = null;
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
