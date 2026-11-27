import {
  type DockviewApi,
  DockviewReact,
  type DockviewReadyEvent,
  themeDark,
  themeLight,
} from "dockview-react";
import { FileText, RotateCcw, X as XIcon } from "lucide-react";
import {
  memo,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import type {
  DiskStatus,
  DraftStore,
  FlashStore,
} from "@/features/document/use-document";
import type { WeftDeclKind } from "@/features/editor/weft-ls-client";
import { type Lang, strings } from "@/i18n";
import type { WeftDoc } from "@/lib/document";
import { settings } from "@/lib/settings";
import type { ResolvedTheme } from "@/lib/theme";
import { DOCUMENT_TONE_STYLE, documentToneOf } from "@/lib/ui";
import {
  type EditorRevealRequest,
  WorkspaceDocContext,
  WorkspaceEditorContext,
  WorkspaceMetaContext,
  WorkspaceThemeContext,
  WorkspaceToneContext,
} from "./workspace-context";
import {
  applyDefaultLayout,
  focusOrOpenPanel,
  panelKindOf,
  panelTitle,
  restoreLayout,
  WORKSPACE_COMPONENTS,
  WorkspaceAddTabAction,
  WorkspaceGroupActions,
  WorkspaceTab,
  WorkspaceWatermark,
} from "./workspace-panels";
import "dockview-react/dist/styles/dockview.css";

const LAYOUT_SETTINGS_KEY = "workspaceLayout:v1";

const FOOTER_FILE_ICON = (
  <FileText className="size-3.5 shrink-0" aria-hidden="true" />
);
const FOOTER_RESET_ICON = <RotateCcw className="size-3.5" />;
const FOOTER_CLOSE_ICON = <XIcon className="size-3.5" />;

// 隔离 Dockview 宿主：flash / diagnostic / dirty 等状态更新时不触发 DockviewReact 重渲染
const WorkspaceDockview = memo(function WorkspaceDockview({
  theme,
  onReady,
}: {
  theme: ResolvedTheme;
  onReady: (event: DockviewReadyEvent) => void;
}) {
  return (
    <DockviewReact
      theme={theme === "dark" ? themeDark : themeLight}
      disableDnd
      disableFloatingGroups
      components={WORKSPACE_COMPONENTS}
      defaultTabComponent={WorkspaceTab}
      leftHeaderActionsComponent={WorkspaceAddTabAction}
      rightHeaderActionsComponent={WorkspaceGroupActions}
      watermarkComponent={WorkspaceWatermark}
      onReady={onReady}
    />
  );
});

// 仅底栏订阅保存闪烁状态，避免保存瞬间引起整个 Workspace 重渲染
const WorkspaceFooter = memo(function WorkspaceFooter({
  path,
  flashStore,
  dirty,
  diskStatus,
  lang,
  onResetLayout,
  onClose,
}: {
  path: string;
  flashStore: FlashStore;
  dirty: boolean;
  diskStatus: DiskStatus;
  lang: Lang;
  onResetLayout: () => void;
  onClose: () => void;
}) {
  const flash = useSyncExternalStore(
    flashStore.subscribe,
    flashStore.getSnapshot,
  );
  const t = strings[lang];
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
    <footer
      className={`flex h-8 shrink-0 items-center gap-2 border-t border-base-300 px-3 text-xs text-base-content/60 transition-colors ${
        flash ? "bg-base-300" : "bg-base-200"
      }`}
    >
      {FOOTER_FILE_ICON}
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
        onClick={onResetLayout}
      >
        {FOOTER_RESET_ICON}
      </button>
      <button
        type="button"
        className="btn btn-ghost btn-xs btn-square"
        aria-label={t.close}
        title={t.close}
        onClick={onClose}
      >
        {FOOTER_CLOSE_ICON}
      </button>
    </footer>
  );
});

export function Workspace({
  path,
  doc,
  draftStore,
  flashStore,
  dirty,
  saving,
  diskStatus,
  lang,
  theme,
  formatOnSave,
  onDraftChange,
  onSave,
  onForceSave,
  onReloadFromDisk,
  onClose,
}: {
  path: string;
  doc: WeftDoc | null;
  draftStore: DraftStore;
  flashStore: FlashStore;
  dirty: boolean;
  saving: boolean;
  diskStatus: DiskStatus;
  lang: Lang;
  theme: ResolvedTheme;
  formatOnSave: boolean;
  onDraftChange: (docLength: number, read: () => string) => void;
  onSave: () => Promise<boolean>;
  onForceSave: () => Promise<boolean>;
  onReloadFromDisk: () => Promise<void>;
  onClose: () => void;
}) {
  const apiRef = useRef<DockviewApi | null>(null);
  const cleanupDockRef = useRef<(() => void) | null>(null);
  const langRef = useRef(lang);
  const restoringLayout = useRef(false);
  const saveActionRef = useRef({
    dirty,
    saving,
    diskStatus,
    formatOnSave,
    onSave,
  });
  const persistTimer = useRef<ReturnType<typeof setTimeout> | undefined>(
    undefined,
  );
  // 挂载即预取布局，与 Dockview 初始化并行消除 onReady 瀑布
  const [initialLayoutPromise] = useState(() =>
    settings.get<unknown>(LAYOUT_SETTINGS_KEY),
  );
  const initialLayoutConsumedRef = useRef(false);

  // 快捷键回调读最新动作：渲染期不写 ref，放到提交阶段同步
  useEffect(() => {
    saveActionRef.current = { dirty, saving, diskStatus, formatOnSave, onSave };
  }, [dirty, saving, diskStatus, formatOnSave, onSave]);

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
          formatOnSave: isFormatOnSave,
          onSave: save,
        } = saveActionRef.current;
        if ((isDirty || status === "deleted" || isFormatOnSave) && !isSaving) {
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

      const layoutPromise = initialLayoutConsumedRef.current
        ? settings.get<unknown>(LAYOUT_SETTINGS_KEY)
        : initialLayoutPromise;
      initialLayoutConsumedRef.current = true;

      void layoutPromise
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
    [initialLayoutPromise, schedulePersistLayout],
  );

  const [revealRequest, setRevealRequest] =
    useState<EditorRevealRequest | null>(null);
  const revealSeqRef = useRef(0);

  const revealInEditor = useCallback((kind: WeftDeclKind, name: string) => {
    revealSeqRef.current += 1;
    setRevealRequest({ kind, name, seq: revealSeqRef.current });
    const api = apiRef.current;
    if (api) {
      focusOrOpenPanel(api, "editor", langRef.current);
    }
  }, []);

  const metaContextValue = useMemo(
    () => ({ path, lang, revealInEditor }),
    [path, lang, revealInEditor],
  );

  const editorContextValue = useMemo(
    () => ({
      draftStore,
      dirty,
      saving,
      diskStatus,
      revealRequest,
      onDraftChange,
      onSave,
      onForceSave,
      onReloadFromDisk,
    }),
    [
      draftStore,
      dirty,
      saving,
      diskStatus,
      revealRequest,
      onDraftChange,
      onSave,
      onForceSave,
      onReloadFromDisk,
    ],
  );

  const handleResetLayout = useCallback(() => {
    const api = apiRef.current;
    if (!api) return;
    applyDefaultLayout(api, langRef.current);
    schedulePersistLayout(api);
  }, [schedulePersistLayout]);

  const tone = documentToneOf(dirty, diskStatus);

  return (
    <div
      className="flex h-full flex-col bg-base-100 text-base-content"
      lang={lang === "zh" ? "zh-CN" : "en"}
    >
      <main className="min-h-0 flex-1 overflow-hidden">
        <WorkspaceMetaContext.Provider value={metaContextValue}>
          <WorkspaceDocContext.Provider value={doc}>
            <WorkspaceEditorContext.Provider value={editorContextValue}>
              <WorkspaceToneContext.Provider value={tone}>
                <WorkspaceThemeContext.Provider value={theme}>
                  <WorkspaceDockview theme={theme} onReady={onReady} />
                </WorkspaceThemeContext.Provider>
              </WorkspaceToneContext.Provider>
            </WorkspaceEditorContext.Provider>
          </WorkspaceDocContext.Provider>
        </WorkspaceMetaContext.Provider>
      </main>

      <WorkspaceFooter
        path={path}
        flashStore={flashStore}
        dirty={dirty}
        diskStatus={diskStatus}
        lang={lang}
        onResetLayout={handleResetLayout}
        onClose={onClose}
      />
    </div>
  );
}
