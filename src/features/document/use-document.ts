import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useRef, useState } from "react";
import type { Lang } from "@/i18n";
import type { WeftDoc } from "@/lib/document";

export type DiskStatus = "synced" | "modified" | "deleted";

type CompileResult =
  | { status: "ok"; doc: WeftDoc }
  | { status: "invalid" }
  | { status: "failed"; message: string };

type OpenDocumentResult = {
  content: string;
  compile: CompileResult;
};

type DiskInspection = {
  file:
    | { status: "unchanged" }
    | { status: "modified"; content: string }
    | { status: "deleted" };
  scriptsChanged: boolean;
};

type SaveResult =
  | { status: "saved"; content: string; compile: CompileResult }
  | { status: "conflict"; diskContent: string };

/**
 * 编辑器草稿容器：
 * - `setFromEditor` 仅记录最新草稿，不通知外部订阅者，按键输入零 React 重渲染；
 * - `setExternal` 用于打开/重载/保存格式化回写，通知 CodeMirror 同步文档。
 */
export type DraftStore = {
  getSnapshot: () => string;
  subscribeExternal: (listener: () => void) => () => void;
};

export type FlashStore = {
  getSnapshot: () => boolean;
  subscribe: (listener: () => void) => () => void;
};

function createDraftStore() {
  let value = "";
  let pendingReader: (() => string) | null = null;
  const externalListeners = new Set<() => void>();

  return {
    getSnapshot: () => {
      if (pendingReader) {
        value = pendingReader();
        pendingReader = null;
      }
      return value;
    },
    subscribeExternal: (listener: () => void) => {
      externalListeners.add(listener);
      return () => {
        externalListeners.delete(listener);
      };
    },
    setFromEditor: (read: () => string) => {
      pendingReader = read;
    },
    setExternal: (next: string) => {
      pendingReader = null;
      value = next;
      for (const listener of externalListeners) listener();
    },
  };
}

function createFlashStore() {
  let value = false;
  const listeners = new Set<() => void>();

  return {
    getSnapshot: () => value,
    subscribe: (listener: () => void) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    set: (next: boolean) => {
      if (next === value) return;
      value = next;
      for (const listener of listeners) listener();
    },
  };
}

const WEFT_FILTER = { name: "WEFT", extensions: ["weft"] };

// 编辑草稿与已保存视图分离：草稿诊断由 @codemirror/lsp-client 自动维护，
// 保存后才编译更新可视化视图；外部文件与 .rn 脚本变更在窗口回到焦点及保存前探测。
export function useDocument(lang: Lang) {
  const [path, setPath] = useState<string | null>(null);
  const [doc, setDoc] = useState<WeftDoc | null>(null);
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [diskStatus, setDiskStatus] = useState<DiskStatus>("synced");

  const [draftStore] = useState(createDraftStore);
  const [flashStore] = useState(createFlashStore);

  const mountedRef = useRef(false);
  const pathRef = useRef<string | null>(null);
  const savedContentRef = useRef<string | null>(null);
  const busyRef = useRef(false);
  const savingRef = useRef(false);
  const inspectingRef = useRef(false);
  const sessionSeq = useRef(0);
  const docCompileSeq = useRef(0);
  const prevLangRef = useRef(lang);
  const flashTimer = useRef<ReturnType<typeof setTimeout> | undefined>(
    undefined,
  );

  const triggerFlash = useCallback(() => {
    flashStore.set(true);
    clearTimeout(flashTimer.current);
    flashTimer.current = setTimeout(() => flashStore.set(false), 400);
  }, [flashStore]);

  const applyPublishedCompile = useCallback(
    (result: CompileResult, options: { flashOnOk?: boolean } = {}) => {
      if (result.status === "ok") {
        setDoc(result.doc);
        if (options.flashOnOk) triggerFlash();
        return;
      }
      setDoc(null);
      if (result.status === "failed") {
        console.error("compile_document failed:", result.message);
      }
    },
    [triggerFlash],
  );

  const recompileSaved = useCallback(
    async (session: number, options: { flashOnOk?: boolean } = {}) => {
      const saved = savedContentRef.current;
      if (saved === null) return;
      const seq = ++docCompileSeq.current;
      try {
        const result = await invoke<CompileResult>("compile_document", {
          content: saved,
        });
        if (
          !mountedRef.current ||
          sessionSeq.current !== session ||
          docCompileSeq.current !== seq
        ) {
          return;
        }
        applyPublishedCompile(result, { flashOnOk: options.flashOnOk });
      } catch (error) {
        console.error("compile_document failed:", error);
      }
    },
    [applyPublishedCompile],
  );

  const reloadInternal = useCallback(
    async (session: number, options: { flashOnOk?: boolean } = {}) => {
      const docSeq = ++docCompileSeq.current;
      const result = await invoke<OpenDocumentResult>("reload_document");
      if (
        !mountedRef.current ||
        sessionSeq.current !== session ||
        docCompileSeq.current !== docSeq
      ) {
        return;
      }
      savedContentRef.current = result.content;
      draftStore.setExternal(result.content);
      setDirty(false);
      setDiskStatus("synced");
      applyPublishedCompile(result.compile, { flashOnOk: options.flashOnOk });
    },
    [applyPublishedCompile, draftStore],
  );

  const inspectDisk = useCallback(async () => {
    if (
      !mountedRef.current ||
      pathRef.current === null ||
      busyRef.current ||
      savingRef.current ||
      inspectingRef.current
    ) {
      return;
    }
    const session = sessionSeq.current;
    inspectingRef.current = true;
    try {
      const result = await invoke<DiskInspection>("inspect_disk");
      if (!mountedRef.current || sessionSeq.current !== session) return;

      const currentDraft = draftStore.getSnapshot();
      const isDirty =
        savedContentRef.current !== null &&
        currentDraft !== savedContentRef.current;

      if (result.file.status === "deleted") {
        setDiskStatus("deleted");
        if (result.scriptsChanged) {
          void recompileSaved(session, { flashOnOk: true });
        }
        return;
      }

      if (result.file.status === "modified") {
        if (!isDirty || result.file.content === currentDraft) {
          await reloadInternal(session, { flashOnOk: true });
          return;
        }
        setDiskStatus("modified");
        if (result.scriptsChanged) {
          void recompileSaved(session);
        }
        return;
      }

      setDiskStatus("synced");
      if (result.scriptsChanged) {
        void recompileSaved(session, { flashOnOk: true });
      }
    } catch (error) {
      console.error("inspect_disk failed:", error);
    } finally {
      inspectingRef.current = false;
    }
  }, [draftStore, recompileSaved, reloadInternal]);

  const inspectDiskRef = useRef(inspectDisk);
  useEffect(() => {
    inspectDiskRef.current = inspectDisk;
  }, [inspectDisk]);

  useEffect(() => {
    mountedRef.current = true;
    void invoke("close_document").catch(() => {});

    const handleFocus = () => {
      void inspectDiskRef.current();
    };
    const handleVisibility = () => {
      if (document.visibilityState === "visible") {
        void inspectDiskRef.current();
      }
    };
    window.addEventListener("focus", handleFocus);
    document.addEventListener("visibilitychange", handleVisibility);

    return () => {
      mountedRef.current = false;
      window.removeEventListener("focus", handleFocus);
      document.removeEventListener("visibilitychange", handleVisibility);
      clearTimeout(flashTimer.current);
      if (pathRef.current !== null || busyRef.current) {
        pathRef.current = null;
        void invoke("close_document").catch(() => {});
      }
    };
  }, []);

  useEffect(() => {
    if (prevLangRef.current === lang) return;
    prevLangRef.current = lang;
    if (pathRef.current === null || busyRef.current) return;
    void recompileSaved(sessionSeq.current);
  }, [lang, recompileSaved]);

  const bind = useCallback(
    async (nextPath: string): Promise<"bound" | "failed" | "ignored"> => {
      if (busyRef.current) return "ignored";
      busyRef.current = true;
      const session = ++sessionSeq.current;
      docCompileSeq.current += 1;
      try {
        const result = await invoke<OpenDocumentResult>("open_document", {
          path: nextPath,
        });
        if (!mountedRef.current || sessionSeq.current !== session) {
          void invoke("close_document").catch(() => {});
          return "ignored";
        }
        pathRef.current = nextPath;
        savedContentRef.current = result.content;
        draftStore.setExternal(result.content);
        setPath(nextPath);
        setDirty(false);
        setDiskStatus("synced");
        clearTimeout(flashTimer.current);
        flashStore.set(false);
        applyPublishedCompile(result.compile);
        return "bound";
      } catch (error) {
        console.error("open_document failed:", error);
        return "failed";
      } finally {
        busyRef.current = false;
      }
    },
    [applyPublishedCompile, draftStore, flashStore],
  );

  const updateDraft = useCallback(
    (docLength: number, read: () => string) => {
      draftStore.setFromEditor(read);
      const saved = savedContentRef.current;
      if (saved === null) {
        setDirty(false);
        return;
      }
      if (docLength !== saved.length) {
        setDirty(true);
        return;
      }
      setDirty(draftStore.getSnapshot() !== saved);
    },
    [draftStore],
  );

  const saveWithMode = useCallback(
    async (force: boolean): Promise<boolean> => {
      if (
        busyRef.current ||
        savingRef.current ||
        pathRef.current === null ||
        savedContentRef.current === null
      ) {
        return false;
      }
      const session = sessionSeq.current;
      const content = draftStore.getSnapshot();
      const docSeq = ++docCompileSeq.current;
      savingRef.current = true;
      setSaving(true);
      try {
        const result = await invoke<SaveResult>("save_document", {
          content,
          force,
        });
        if (!mountedRef.current || sessionSeq.current !== session) {
          return false;
        }
        if (result.status === "conflict") {
          setDiskStatus("modified");
          return false;
        }
        savedContentRef.current = result.content;
        if (
          result.content !== content &&
          draftStore.getSnapshot() === content
        ) {
          draftStore.setExternal(result.content);
        }
        const currentDraft = draftStore.getSnapshot();
        setDirty(currentDraft !== result.content);
        setDiskStatus("synced");
        if (docCompileSeq.current === docSeq) {
          applyPublishedCompile(result.compile, { flashOnOk: true });
        }
        return true;
      } catch (error) {
        console.error("save_document failed:", error);
        return false;
      } finally {
        savingRef.current = false;
        if (mountedRef.current) setSaving(false);
      }
    },
    [applyPublishedCompile, draftStore],
  );

  const save = useCallback(() => saveWithMode(false), [saveWithMode]);
  const forceSave = useCallback(() => saveWithMode(true), [saveWithMode]);

  const reloadFromDisk = useCallback(async () => {
    if (busyRef.current || savingRef.current || pathRef.current === null) {
      return;
    }
    busyRef.current = true;
    try {
      await reloadInternal(sessionSeq.current, { flashOnOk: true });
    } catch (error) {
      console.error("reload_document failed:", error);
    } finally {
      busyRef.current = false;
    }
  }, [reloadInternal]);

  const openFile = useCallback(async (): Promise<string | null> => {
    try {
      const selected = await open({ multiple: false, filters: [WEFT_FILTER] });
      return typeof selected === "string" ? selected : null;
    } catch (error) {
      console.error("openFile failed:", error);
      return null;
    }
  }, []);

  const close = useCallback(async () => {
    if (busyRef.current) return;
    busyRef.current = true;
    sessionSeq.current += 1;
    docCompileSeq.current += 1;
    try {
      await invoke("close_document");
      if (!mountedRef.current) return;
      pathRef.current = null;
      savedContentRef.current = null;
      draftStore.setExternal("");
      savingRef.current = false;
      setPath(null);
      setDoc(null);
      setDirty(false);
      setSaving(false);
      setDiskStatus("synced");
      clearTimeout(flashTimer.current);
      flashStore.set(false);
    } catch (error) {
      console.error("close_document failed:", error);
    } finally {
      busyRef.current = false;
    }
  }, [draftStore, flashStore]);

  return {
    path,
    doc,
    draftStore,
    flashStore,
    dirty,
    saving,
    diskStatus,
    bind,
    updateDraft,
    save,
    forceSave,
    reloadFromDisk,
    openFile,
    close,
  };
}
