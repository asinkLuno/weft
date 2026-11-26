import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useRef, useState } from "react";
import type { CompileDiagnostic, Lang } from "@/i18n";
import type { WeftDoc } from "@/lib/document";

export type DiskStatus = "synced" | "modified" | "deleted";

type CompileResult =
  | { status: "ok"; doc: WeftDoc }
  | { status: "invalid"; diagnostic: CompileDiagnostic }
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

export type DraftStore = {
  getSnapshot: () => string;
  subscribe: (listener: () => void) => () => void;
};

function createDraftStore() {
  let value = "";
  const listeners = new Set<() => void>();

  return {
    getSnapshot: () => value,
    subscribe: (listener: () => void) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    set: (next: string) => {
      if (next === value) return;
      value = next;
      for (const listener of listeners) listener();
    },
  };
}

const WEFT_FILTER = { name: "Weft", extensions: ["weft"] };
const CHECK_DEBOUNCE_MS = 250;

// 编辑草稿与已保存视图分离：输入期间只做内联诊断，保存后才更新渲染视图；
// 外部文件与 .rn 脚本变更在窗口回到焦点及保存前探测，不运行后台 watcher。
export function useDocument(lang: Lang) {
  const [path, setPath] = useState<string | null>(null);
  const [doc, setDoc] = useState<WeftDoc | null>(null);
  const [diagnostic, setDiagnostic] = useState<CompileDiagnostic | null>(null);
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [diskStatus, setDiskStatus] = useState<DiskStatus>("synced");
  const [flash, setFlash] = useState(false);

  const [draftStore] = useState(createDraftStore);

  const mountedRef = useRef(false);
  const pathRef = useRef<string | null>(null);
  const savedContentRef = useRef<string | null>(null);
  const busyRef = useRef(false);
  const savingRef = useRef(false);
  const inspectingRef = useRef(false);
  const sessionSeq = useRef(0);
  const docCompileSeq = useRef(0);
  const checkSeq = useRef(0);
  const prevLangRef = useRef(lang);
  const checkTimer = useRef<ReturnType<typeof setTimeout> | undefined>(
    undefined,
  );
  const flashTimer = useRef<ReturnType<typeof setTimeout> | undefined>(
    undefined,
  );

  const triggerFlash = useCallback(() => {
    setFlash(true);
    clearTimeout(flashTimer.current);
    flashTimer.current = setTimeout(() => setFlash(false), 400);
  }, []);

  const applyPublishedCompile = useCallback(
    (
      result: CompileResult,
      options: { updateDiagnostic: boolean; flashOnOk?: boolean },
    ) => {
      if (result.status === "ok") {
        setDoc(result.doc);
        if (options.updateDiagnostic) setDiagnostic(null);
        if (options.flashOnOk) triggerFlash();
        return;
      }
      setDoc(null);
      if (result.status === "invalid") {
        if (options.updateDiagnostic) setDiagnostic(result.diagnostic);
        return;
      }
      console.error("compile_document failed:", result.message);
      if (options.updateDiagnostic) setDiagnostic(null);
    },
    [triggerFlash],
  );

  const checkDraft = useCallback(async (content: string, session: number) => {
    const seq = ++checkSeq.current;
    try {
      const next = await invoke<CompileDiagnostic | null>("check_document", {
        content,
      });
      if (
        !mountedRef.current ||
        sessionSeq.current !== session ||
        checkSeq.current !== seq
      ) {
        return;
      }
      setDiagnostic(next);
    } catch (error) {
      console.error("check_document failed:", error);
    }
  }, []);

  // 重新编译已保存的文档内容以刷新视图；若草稿未保存则同时刷新草稿诊断。
  const recompileSaved = useCallback(
    async (session: number, options: { flashOnOk?: boolean } = {}) => {
      const saved = savedContentRef.current;
      if (saved === null) return;
      const currentDraft = draftStore.getSnapshot();
      const isDirty = currentDraft !== saved;

      const seq = ++docCompileSeq.current;
      clearTimeout(checkTimer.current);
      if (isDirty) {
        // 与 compile_document 并行执行草稿诊断，避免串行等待；updateDraft 会自增 checkSeq 挡住过期结果
        void checkDraft(currentDraft, session);
      } else {
        checkSeq.current += 1;
      }

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
        const stillClean = draftStore.getSnapshot() === saved;
        applyPublishedCompile(result, {
          updateDiagnostic: stillClean,
          flashOnOk: options.flashOnOk,
        });
      } catch (error) {
        console.error("compile_document failed:", error);
      }
    },
    [applyPublishedCompile, checkDraft, draftStore],
  );

  const reloadInternal = useCallback(
    async (session: number, options: { flashOnOk?: boolean } = {}) => {
      clearTimeout(checkTimer.current);
      const docSeq = ++docCompileSeq.current;
      const diagSeq = ++checkSeq.current;
      const result = await invoke<OpenDocumentResult>("reload_document");
      if (
        !mountedRef.current ||
        sessionSeq.current !== session ||
        docCompileSeq.current !== docSeq ||
        checkSeq.current !== diagSeq
      ) {
        return;
      }
      savedContentRef.current = result.content;
      draftStore.set(result.content);
      setDirty(false);
      setDiskStatus("synced");
      applyPublishedCompile(result.compile, {
        updateDiagnostic: true,
        flashOnOk: options.flashOnOk,
      });
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

  // 挂载时清理页面刷新/HMR 前可能残留的后端会话，并在窗口恢复焦点时探测外部改动。
  useEffect(() => {
    mountedRef.current = true;
    void invoke("close_document").catch(() => {});

    const handleFocus = () => {
      void inspectDisk();
    };
    const handleVisibility = () => {
      if (document.visibilityState === "visible") {
        void inspectDisk();
      }
    };
    window.addEventListener("focus", handleFocus);
    document.addEventListener("visibilitychange", handleVisibility);

    return () => {
      mountedRef.current = false;
      window.removeEventListener("focus", handleFocus);
      document.removeEventListener("visibilitychange", handleVisibility);
      clearTimeout(checkTimer.current);
      clearTimeout(flashTimer.current);
      if (pathRef.current !== null || busyRef.current) {
        pathRef.current = null;
        void invoke("close_document").catch(() => {});
      }
    };
  }, [inspectDisk]);

  // 语言切换后重编已保存文档刷新本地化展示值，未保存草稿同步刷新诊断。
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
      clearTimeout(checkTimer.current);
      const session = ++sessionSeq.current;
      docCompileSeq.current += 1;
      checkSeq.current += 1;
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
        draftStore.set(result.content);
        setPath(nextPath);
        setDirty(false);
        setDiskStatus("synced");
        clearTimeout(flashTimer.current);
        setFlash(false);
        applyPublishedCompile(result.compile, { updateDiagnostic: true });
        return "bound";
      } catch (error) {
        console.error("open_document failed:", error);
        return "failed";
      } finally {
        busyRef.current = false;
      }
    },
    [applyPublishedCompile, draftStore],
  );

  const updateDraft = useCallback(
    (next: string) => {
      draftStore.set(next);
      const saved = savedContentRef.current;
      setDirty(saved !== null && next !== saved);
      clearTimeout(checkTimer.current);
      checkSeq.current += 1;
      const session = sessionSeq.current;
      checkTimer.current = setTimeout(() => {
        void checkDraft(next, session);
      }, CHECK_DEBOUNCE_MS);
    },
    [checkDraft, draftStore],
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
      clearTimeout(checkTimer.current);
      const docSeq = ++docCompileSeq.current;
      const diagSeq = ++checkSeq.current;
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
        const currentDraft = draftStore.getSnapshot();
        setDirty(currentDraft !== result.content);
        setDiskStatus("synced");
        if (docCompileSeq.current === docSeq) {
          const updateDiag =
            checkSeq.current === diagSeq && currentDraft === result.content;
          applyPublishedCompile(result.compile, {
            updateDiagnostic: updateDiag,
            flashOnOk: true,
          });
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
    clearTimeout(checkTimer.current);
    sessionSeq.current += 1;
    docCompileSeq.current += 1;
    checkSeq.current += 1;
    try {
      await invoke("close_document");
      if (!mountedRef.current) return;
      pathRef.current = null;
      savedContentRef.current = null;
      draftStore.set("");
      savingRef.current = false;
      setPath(null);
      setDoc(null);
      setDiagnostic(null);
      setDirty(false);
      setSaving(false);
      setDiskStatus("synced");
      clearTimeout(flashTimer.current);
      setFlash(false);
    } catch (error) {
      console.error("close_document failed:", error);
    } finally {
      busyRef.current = false;
    }
  }, [draftStore]);

  return {
    path,
    doc,
    diagnostic,
    draftStore,
    dirty,
    saving,
    diskStatus,
    flash,
    bind,
    updateDraft,
    save,
    forceSave,
    reloadFromDisk,
    openFile,
    close,
  };
}
