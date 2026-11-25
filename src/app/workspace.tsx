import {
  type DockviewApi,
  DockviewReact,
  type DockviewReadyEvent,
} from "dockview-react";
import { FileText, RotateCcw, X as XIcon } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef } from "react";
import { Button } from "@/components/ui/button";
import type { DraftStore } from "@/features/document/use-document";
import { type Lang, strings } from "@/i18n";
import type { WeftDoc } from "@/lib/document";
import { settings } from "@/lib/settings";
import {
  WorkspaceDocContext,
  WorkspaceEditorContext,
} from "./workspace-context";
import {
  applyDefaultLayout,
  panelKindOf,
  panelTitle,
  restoreLayout,
  WEFT_DOCKVIEW_THEME,
  WORKSPACE_COMPONENTS,
  WorkspaceGroupActions,
  WorkspaceTab,
  WorkspaceWatermark,
} from "./workspace-panels";
import "./dockview-theme.css";
import "./workspace.css";

const LAYOUT_SETTINGS_KEY = "workspaceLayout:v1";

export function Workspace({
  path,
  doc,
  draftStore,
  dirty,
  saving,
  hasExternalChange,
  lang,
  flash,
  activeError,
  onDraftChange,
  onSave,
  onReloadFromDisk,
  onClose,
}: {
  path: string;
  doc: WeftDoc | null;
  draftStore: DraftStore;
  dirty: boolean;
  saving: boolean;
  hasExternalChange: boolean;
  lang: Lang;
  flash: boolean;
  activeError: unknown;
  onDraftChange: (next: string) => void;
  onSave: () => Promise<boolean>;
  onReloadFromDisk: () => void;
  onClose: () => void;
}) {
  const t = strings[lang];
  const apiRef = useRef<DockviewApi | null>(null);
  const cleanupDockRef = useRef<(() => void) | null>(null);
  const langRef = useRef(lang);
  const restoringLayout = useRef(false);
  const saveActionRef = useRef({ dirty, saving, onSave });
  const persistTimer = useRef<ReturnType<typeof setTimeout> | undefined>(
    undefined,
  );

  saveActionRef.current = { dirty, saving, onSave };

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

  // 全局 Ctrl/Cmd+S 兜底：焦点不在编辑器面板内时也可保存；CodeMirror 已处理时会 preventDefault。
  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.defaultPrevented) return;
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "s") {
        event.preventDefault();
        const {
          dirty: isDirty,
          saving: isSaving,
          onSave: save,
        } = saveActionRef.current;
        if (isDirty && !isSaving) {
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
      dirty,
      saving,
      hasExternalChange,
      onDraftChange,
      onSave,
      onReloadFromDisk,
    }),
    [
      draftStore,
      dirty,
      saving,
      hasExternalChange,
      onDraftChange,
      onSave,
      onReloadFromDisk,
    ],
  );

  function handleResetLayout() {
    const api = apiRef.current;
    if (!api) return;
    applyDefaultLayout(api, lang);
    schedulePersistLayout(api);
  }

  return (
    <div className="app" lang={lang === "zh" ? "zh-CN" : "en"}>
      <main className="workspace-dock">
        <WorkspaceDocContext.Provider value={docContextValue}>
          <WorkspaceEditorContext.Provider value={editorContextValue}>
            <DockviewReact
              theme={WEFT_DOCKVIEW_THEME}
              disableDnd
              disableFloatingGroups
              components={WORKSPACE_COMPONENTS}
              defaultTabComponent={WorkspaceTab}
              rightHeaderActionsComponent={WorkspaceGroupActions}
              watermarkComponent={WorkspaceWatermark}
              onReady={onReady}
            />
          </WorkspaceEditorContext.Provider>
        </WorkspaceDocContext.Provider>
      </main>

      <footer className={flash ? "statusbar flash" : "statusbar"}>
        <FileText className="size-3.5 shrink-0" aria-hidden="true" />
        <span className="statusbar-path" title={path}>
          {path}
        </span>
        <span
          className={`statusbar-state${activeError ? " has-error" : dirty ? " is-dirty" : ""}`}
        >
          {activeError ? t.fileError : dirty ? t.unsaved : t.fileWatching}
        </span>
        <Button
          variant="ghost"
          size="icon-xs"
          className="shrink-0 text-muted-foreground"
          aria-label={t.resetLayout}
          title={t.resetLayout}
          onClick={handleResetLayout}
        >
          <RotateCcw />
        </Button>
        <Button
          variant="ghost"
          size="icon-xs"
          className="shrink-0 text-muted-foreground"
          aria-label={t.close}
          title={t.close}
          onClick={onClose}
        >
          <XIcon />
        </Button>
      </footer>
    </div>
  );
}
