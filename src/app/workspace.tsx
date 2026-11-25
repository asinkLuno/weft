import {
  type DockviewApi,
  DockviewReact,
  type DockviewReadyEvent,
  themeLight,
} from "dockview-react";
import { FileText, RotateCcw, X as XIcon } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef } from "react";
import type { DiskStatus, DraftStore } from "@/features/document/use-document";
import { type CompileDiagnostic, type Lang, strings } from "@/i18n";
import type { WeftDoc } from "@/lib/document";
import { settings } from "@/lib/settings";
import { DOCUMENT_TONE_STYLE, documentToneOf } from "@/lib/ui";
import {
  WorkspaceDocContext,
  WorkspaceEditorContext,
} from "./workspace-context";
import {
  applyDefaultLayout,
  panelKindOf,
  panelTitle,
  restoreLayout,
  WORKSPACE_COMPONENTS,
  WorkspaceAddTabAction,
  WorkspaceTab,
  WorkspaceWatermark,
} from "./workspace-panels";
import "dockview-react/dist/styles/dockview.css";

const LAYOUT_SETTINGS_KEY = "workspaceLayout:v1";

export function Workspace({
  path,
  doc,
  diagnostic,
  draftStore,
  dirty,
  saving,
  diskStatus,
  lang,
  flash,
  onDraftChange,
  onSave,
  onForceSave,
  onReloadFromDisk,
  onClose,
}: {
  path: string;
  doc: WeftDoc | null;
  diagnostic: CompileDiagnostic | null;
  draftStore: DraftStore;
  dirty: boolean;
  saving: boolean;
  diskStatus: DiskStatus;
  lang: Lang;
  flash: boolean;
  onDraftChange: (next: string) => void;
  onSave: () => Promise<boolean>;
  onForceSave: () => Promise<boolean>;
  onReloadFromDisk: () => Promise<void>;
  onClose: () => void;
}) {
  const t = strings[lang];
  const apiRef = useRef<DockviewApi | null>(null);
  const cleanupDockRef = useRef<(() => void) | null>(null);
  const langRef = useRef(lang);
  const restoringLayout = useRef(false);
  const saveActionRef = useRef({ dirty, saving, diskStatus, onSave });
  const persistTimer = useRef<ReturnType<typeof setTimeout> | undefined>(
    undefined,
  );

  // 快捷键回调读最新动作：渲染期不写 ref，放到提交阶段同步
  useEffect(() => {
    saveActionRef.current = { dirty, saving, diskStatus, onSave };
  });

  useEffect(() => {
    langRef.current = lang;
    const api = apiRef.current;
    if (!api) return;
    for (const panel of api.panels) {
      const kind = panelKindOf(panel);
      if (kind) {
        panel.api.setTitle(panelTitle(kind, lang));
      }
    }
  }, [lang]);

  // 全局 Ctrl/Cmd+S 保存（覆盖编辑器与其余面板）
  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.defaultPrevented) return;
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "s") {
        event.preventDefault();
        const {
          dirty: isDirty,
          saving: isSaving,
          diskStatus: status,
          onSave: save,
        } = saveActionRef.current;
        if ((isDirty || status === "deleted") && !isSaving) {
          void save();
        }
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, []);

  useEffect(() => {
    return () => {
      clearTimeout(persistTimer.current);
      cleanupDockRef.current?.();
      cleanupDockRef.current = null;
      apiRef.current = null;
    };
  }, []);

  const schedulePersistLayout = useCallback((api: DockviewApi) => {
    if (restoringLayout.current) return;
    clearTimeout(persistTimer.current);
    persistTimer.current = setTimeout(() => {
      void settings.set(LAYOUT_SETTINGS_KEY, api.toJSON()).catch(console.error);
    }, 300);
  }, []);

  const onReady = useCallback(
    (event: DockviewReadyEvent) => {
      cleanupDockRef.current?.();
      const api = event.api;
      apiRef.current = api;

      const disposable = api.onDidLayoutChange(() =>
        schedulePersistLayout(api),
      );

      restoringLayout.current = true;
      let cancelled = false;
      cleanupDockRef.current = () => {
        cancelled = true;
        disposable.dispose();
      };

      void settings
        .get<unknown>(LAYOUT_SETTINGS_KEY)
        .then((saved) => {
          if (cancelled || apiRef.current !== api) return;
          if (!restoreLayout(api, saved)) {
            applyDefaultLayout(api, langRef.current);
          }
        })
        .catch(() => {
          if (cancelled || apiRef.current !== api) return;
          applyDefaultLayout(api, langRef.current);
        })
        .finally(() => {
          if (!cancelled) {
            restoringLayout.current = false;
          }
        });
    },
    [schedulePersistLayout],
  );

  const docContextValue = useMemo(
    () => ({ path, doc, lang }),
    [path, doc, lang],
  );

  const editorContextValue = useMemo(
    () => ({
      draftStore,
      diagnostic,
      dirty,
      saving,
      diskStatus,
      onDraftChange,
      onSave,
      onForceSave,
      onReloadFromDisk,
    }),
    [
      draftStore,
      diagnostic,
      dirty,
      saving,
      diskStatus,
      onDraftChange,
      onSave,
      onForceSave,
      onReloadFromDisk,
    ],
  );

  function handleResetLayout() {
    const api = apiRef.current;
    if (!api) return;
    applyDefaultLayout(api, lang);
    schedulePersistLayout(api);
  }

  const tone = documentToneOf(dirty, diskStatus);
  const toneStyle = DOCUMENT_TONE_STYLE[tone];
  const isBadgeStatus = tone === "modified" || tone === "deleted";
  const statusLabel =
    diskStatus === "deleted"
      ? t.statusDeleted
      : diskStatus === "modified"
        ? t.statusConflict
        : dirty
          ? t.unsaved
          : t.saved;

  return (
    <div
      className="flex h-full flex-col bg-base-100 text-base-content"
      lang={lang === "zh" ? "zh-CN" : "en"}
    >
      <main className="min-h-0 flex-1 overflow-hidden">
        <WorkspaceDocContext.Provider value={docContextValue}>
          <WorkspaceEditorContext.Provider value={editorContextValue}>
            <DockviewReact
              theme={themeLight}
              disableDnd={false}
              dndStrategy="pointer"
              disableFloatingGroups
              components={WORKSPACE_COMPONENTS}
              defaultTabComponent={WorkspaceTab}
              leftHeaderActionsComponent={WorkspaceAddTabAction}
              watermarkComponent={WorkspaceWatermark}
              onReady={onReady}
            />
          </WorkspaceEditorContext.Provider>
        </WorkspaceDocContext.Provider>
      </main>

      <footer
        className={`flex h-8 shrink-0 items-center gap-2 border-t border-base-300 px-3 text-xs text-base-content/60 transition-colors ${
          flash ? "bg-base-300" : "bg-base-200"
        }`}
      >
        <FileText className="size-3.5 shrink-0" aria-hidden="true" />
        <span className="min-w-0 flex-1 truncate font-mono" title={path}>
          {path}
        </span>
        {isBadgeStatus ? (
          <span className={`shrink-0 whitespace-nowrap ${toneStyle.label}`}>
            {statusLabel}
          </span>
        ) : (
          <span
            className={`inline-flex shrink-0 items-center gap-1.5 whitespace-nowrap ${toneStyle.label}`}
          >
            <span
              className={`size-1.5 rounded-full ${toneStyle.dot}`}
              aria-hidden="true"
            />
            {statusLabel}
          </span>
        )}
        <button
          type="button"
          className="btn btn-ghost btn-xs btn-square"
          aria-label={t.resetLayout}
          title={t.resetLayout}
          onClick={handleResetLayout}
        >
          <RotateCcw className="size-3.5" />
        </button>
        <button
          type="button"
          className="btn btn-ghost btn-xs btn-square"
          aria-label={t.close}
          title={t.close}
          onClick={onClose}
        >
          <XIcon className="size-3.5" />
        </button>
      </footer>
    </div>
  );
}
