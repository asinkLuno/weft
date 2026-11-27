import { closeBrackets, closeBracketsKeymap } from "@codemirror/autocomplete";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import {
  bracketMatching,
  defaultHighlightStyle,
  foldGutter,
  foldKeymap,
  indentOnInput,
  syntaxHighlighting,
} from "@codemirror/language";
import { lintGutter } from "@codemirror/lint";
import { highlightSelectionMatches, searchKeymap } from "@codemirror/search";
import { Compartment, EditorState } from "@codemirror/state";
import { oneDark } from "@codemirror/theme-one-dark";
import {
  drawSelection,
  dropCursor,
  EditorView,
  highlightActiveLine,
  highlightActiveLineGutter,
  highlightSpecialChars,
  keymap,
  lineNumbers,
} from "@codemirror/view";
import {
  AlertCircle,
  AlertTriangle,
  RefreshCw,
  RotateCcw,
  Save,
  Sparkles,
} from "lucide-react";
import {
  memo,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import {
  type EditorRevealRequest,
  useWorkspaceTheme,
} from "@/app/workspace-context";
import type { DiskStatus, DraftStore } from "@/features/document/use-document";
import { type Lang, strings } from "@/i18n";
import { weftLanguage } from "./weft-language";
import {
  createTauriLspTransport,
  createWeftLspClient,
  type DiagnosticCounts,
  formatDocument,
  jumpToDefinition,
  notifyLspLanguage,
  pathToFileUri,
  revealSymbolViaLsp,
} from "./weft-ls-client";

const PATH_SEP_RE = /[\\/]/;

function fileNameOf(filePath: string): string {
  const parts = filePath.split(PATH_SEP_RE);
  return parts[parts.length - 1] || filePath;
}

const REVERT_ICON = <RotateCcw className="size-3.5" />;
const SAVE_ICON = <Save className="size-3.5" />;
const REFRESH_ICON = <RefreshCw className="size-3.5" />;
const FORMAT_ICON = <Sparkles className="size-3.5" />;
const ERROR_BADGE_ICON = <AlertCircle className="size-3" aria-hidden="true" />;
const WARNING_BADGE_ICON = (
  <AlertTriangle className="size-3" aria-hidden="true" />
);

const EMPTY_COUNTS: DiagnosticCounts = { errors: 0, warnings: 0 };

type DiagnosticCountsStore = {
  getSnapshot: () => DiagnosticCounts;
  subscribe: (listener: () => void) => () => void;
  set: (next: DiagnosticCounts) => void;
};

function createDiagnosticCountsStore(): DiagnosticCountsStore {
  let value = EMPTY_COUNTS;
  const listeners = new Set<() => void>();

  return {
    getSnapshot: () => value,
    subscribe: (listener: () => void) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    set: (next: DiagnosticCounts) => {
      if (next.errors === value.errors && next.warnings === value.warnings) {
        return;
      }
      value = next;
      for (const listener of listeners) listener();
    },
  };
}

// 隔离高频诊断计数更新，避免每次 LSP publishDiagnostics 触发整个编辑器视图重渲染
const DiagnosticBadges = memo(function DiagnosticBadges({
  store,
}: {
  store: DiagnosticCountsStore;
}) {
  const counts = useSyncExternalStore(store.subscribe, store.getSnapshot);
  return (
    <>
      {counts.errors > 0 ? (
        <span className="badge badge-xs badge-error gap-1 shrink-0">
          {ERROR_BADGE_ICON}
          {counts.errors}
        </span>
      ) : null}
      {counts.warnings > 0 ? (
        <span className="badge badge-xs badge-warning gap-1 shrink-0">
          {WARNING_BADGE_ICON}
          {counts.warnings}
        </span>
      ) : null}
    </>
  );
});

const BASE_EDITOR_THEME = EditorView.theme({
  "&": { height: "100%" },
  ".cm-scroller": { overflow: "auto" },
});

// 原生 CodeMirror 6 静态基础扩展（替代 @uiw/react-codemirror 包装层）
const BASE_EDITOR_EXTENSIONS = [
  BASE_EDITOR_THEME,
  lineNumbers(),
  highlightActiveLineGutter(),
  highlightSpecialChars(),
  history(),
  foldGutter(),
  drawSelection(),
  dropCursor(),
  indentOnInput(),
  syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
  bracketMatching(),
  closeBrackets(),
  highlightActiveLine(),
  highlightSelectionMatches(),
  keymap.of([
    ...closeBracketsKeymap,
    ...defaultKeymap,
    ...searchKeymap,
    ...historyKeymap,
    ...foldKeymap,
  ]),
  EditorState.tabSize.of(2),
  EditorView.lineWrapping,
  weftLanguage,
  lintGutter(),
];

const WeftCodeMirror = memo(function WeftCodeMirror({
  path,
  lang,
  draftStore,
  revealRequest,
  onChange,
  onDiagnosticCounts,
  onViewReady,
}: {
  path: string;
  lang: Lang;
  draftStore: DraftStore;
  revealRequest: EditorRevealRequest | null;
  onChange: (docLength: number, read: () => string) => void;
  onDiagnosticCounts: (counts: DiagnosticCounts) => void;
  onViewReady: (view: EditorView | null) => void;
}) {
  const theme = useWorkspaceTheme();
  const hostRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const themeCompartmentRef = useRef(new Compartment());
  const handledRevealSeqRef = useRef(0);
  const initialLangRef = useRef(lang);
  const initialThemeRef = useRef(theme);
  const onChangeRef = useRef(onChange);
  const onCountsRef = useRef(onDiagnosticCounts);
  const onViewReadyRef = useRef(onViewReady);

  useEffect(() => {
    onChangeRef.current = onChange;
    onCountsRef.current = onDiagnosticCounts;
    onViewReadyRef.current = onViewReady;
  }, [onChange, onDiagnosticCounts, onViewReady]);

  const fileUri = useMemo(() => pathToFileUri(path), [path]);

  // 每个打开文件持有一个标准 @codemirror/lsp-client 实例（走进程内 Tauri JSON-RPC Transport）
  const lspClient = useMemo(
    () =>
      createWeftLspClient(initialLangRef.current, (counts) =>
        onCountsRef.current(counts),
      ),
    [],
  );

  // 原生挂载 EditorView：打字走非受控更新，零 React 重渲染与整篇字符串比对
  useEffect(() => {
    const parent = hostRef.current;
    if (!parent) return;

    const themeCompartment = themeCompartmentRef.current;
    const view = new EditorView({
      parent,
      state: EditorState.create({
        doc: draftStore.getSnapshot(),
        extensions: [
          ...BASE_EDITOR_EXTENSIONS,
          themeCompartment.of(
            initialThemeRef.current === "dark" ? oneDark : [],
          ),
          lspClient.plugin(fileUri, "weft"),
          EditorView.updateListener.of((update) => {
            if (update.docChanged) {
              const doc = update.state.doc;
              onChangeRef.current(doc.length, () => doc.toString());
            }
          }),
          EditorView.domEventHandlers({
            mousedown(event, targetView) {
              if (!(event.ctrlKey || event.metaKey) || event.button !== 0) {
                return false;
              }
              const pos = targetView.posAtCoords({
                x: event.clientX,
                y: event.clientY,
              });
              if (pos === null) return false;
              targetView.dispatch({ selection: { anchor: pos } });
              return jumpToDefinition(targetView);
            },
          }),
        ],
      }),
    });

    viewRef.current = view;
    onViewReadyRef.current(view);
    lspClient.connect(createTauriLspTransport());

    const unsubscribe = draftStore.subscribeExternal(() => {
      const next = draftStore.getSnapshot();
      if (next === view.state.doc.toString()) return;
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: next },
      });
    });

    return () => {
      unsubscribe();
      lspClient.disconnect();
      onViewReadyRef.current(null);
      viewRef.current = null;
      view.destroy();
    };
  }, [draftStore, fileUri, lspClient]);

  // 动态切换深浅主题，无需重建 EditorView
  useEffect(() => {
    initialThemeRef.current = theme;
    const view = viewRef.current;
    if (!view) return;
    view.dispatch({
      effects: themeCompartmentRef.current.reconfigure(
        theme === "dark" ? oneDark : [],
      ),
    });
  }, [theme]);

  useEffect(() => {
    initialLangRef.current = lang;
    notifyLspLanguage(lspClient, lang);
  }, [lspClient, lang]);

  useEffect(() => {
    const view = viewRef.current;
    if (!view || !revealRequest) return;
    if (revealRequest.seq === handledRevealSeqRef.current) return;
    handledRevealSeqRef.current = revealRequest.seq;
    void revealSymbolViaLsp(
      lspClient,
      view,
      fileUri,
      revealRequest.kind,
      revealRequest.name,
    );
  }, [fileUri, lspClient, revealRequest]);

  return <div ref={hostRef} className="h-full" />;
});

function WeftEditorViewInner({
  path,
  draftStore,
  dirty,
  saving,
  diskStatus,
  revealRequest,
  lang,
  onChange,
  onSave,
  onForceSave,
  onReloadFromDisk,
}: {
  path: string;
  draftStore: DraftStore;
  dirty: boolean;
  saving: boolean;
  diskStatus: DiskStatus;
  revealRequest: EditorRevealRequest | null;
  lang: Lang;
  onChange: (docLength: number, read: () => string) => void;
  onSave: () => Promise<boolean>;
  onForceSave: () => Promise<boolean>;
  onReloadFromDisk: () => Promise<void>;
}) {
  const t = strings[lang];
  const fileName = fileNameOf(path);
  const canSave = (dirty || diskStatus === "deleted") && !saving;
  const viewRef = useRef<EditorView | null>(null);
  const [countsStore] = useState(createDiagnosticCountsStore);

  const handleViewReady = useCallback((view: EditorView | null) => {
    viewRef.current = view;
  }, []);

  const handleFormat = useCallback(() => {
    const view = viewRef.current;
    if (view) {
      formatDocument(view);
    }
  }, []);

  return (
    <div className="flex h-full min-h-0 flex-col bg-base-100">
      <div className="flex h-8 shrink-0 items-center gap-2 border-b border-base-300 bg-base-200 px-3">
        <span
          className="min-w-0 flex-1 truncate font-mono text-xs text-base-content/80"
          title={path}
        >
          {fileName}
        </span>
        <DiagnosticBadges store={countsStore} />
        {dirty ? (
          <span className="badge badge-xs badge-outline shrink-0 border-primary text-primary">
            {t.unsaved}
          </span>
        ) : null}
        <div className="flex shrink-0 items-center gap-1">
          <button
            type="button"
            className="btn btn-ghost btn-xs"
            disabled={saving}
            title="Shift+Alt+F"
            onClick={handleFormat}
          >
            {FORMAT_ICON}
            {t.format}
          </button>
          {dirty && diskStatus !== "deleted" ? (
            <button
              type="button"
              className="btn btn-ghost btn-xs"
              disabled={saving}
              onClick={() => {
                void onReloadFromDisk();
              }}
            >
              {REVERT_ICON}
              {t.revert}
            </button>
          ) : null}
          <button
            type="button"
            className="btn btn-primary btn-xs"
            disabled={!canSave}
            onClick={() => {
              void onSave();
            }}
          >
            {SAVE_ICON}
            {saving ? t.saving : t.save}
          </button>
        </div>
      </div>

      {diskStatus === "modified" ? (
        <div className="flex flex-wrap items-center justify-between gap-2 border-b border-warning bg-warning/25 px-3 py-1.5 text-xs text-warning-content">
          <span>{t.externalChanged}</span>
          <div className="flex items-center gap-1">
            <button
              type="button"
              className="btn btn-ghost btn-xs"
              disabled={saving}
              onClick={() => {
                void onReloadFromDisk();
              }}
            >
              {REFRESH_ICON}
              {t.reloadFromDisk}
            </button>
            <button
              type="button"
              className="btn btn-error btn-xs"
              disabled={saving}
              onClick={() => {
                void onForceSave();
              }}
            >
              {SAVE_ICON}
              {t.overwriteDisk}
            </button>
          </div>
        </div>
      ) : null}

      {diskStatus === "deleted" ? (
        <div className="flex flex-wrap items-center justify-between gap-2 border-b border-error bg-error/25 px-3 py-1.5 text-xs text-error-content">
          <span>{t.externalDeleted}</span>
          <div className="flex items-center gap-1">
            <button
              type="button"
              className="btn btn-primary btn-xs"
              disabled={saving}
              onClick={() => {
                void onForceSave();
              }}
            >
              {SAVE_ICON}
              {t.recreateFile}
            </button>
          </div>
        </div>
      ) : null}

      <div className="min-h-0 flex-1 overflow-hidden">
        <WeftCodeMirror
          path={path}
          lang={lang}
          draftStore={draftStore}
          revealRequest={revealRequest}
          onChange={onChange}
          onDiagnosticCounts={countsStore.set}
          onViewReady={handleViewReady}
        />
      </div>
    </div>
  );
}

export const WeftEditorView = memo(WeftEditorViewInner);
