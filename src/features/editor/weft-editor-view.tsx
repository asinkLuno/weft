import { autocompletion } from "@codemirror/autocomplete";
import {
  type Diagnostic,
  linter,
  lintGutter,
  setDiagnostics,
} from "@codemirror/lint";
import { EditorView } from "@codemirror/view";
import CodeMirror from "@uiw/react-codemirror";
import { RefreshCw, RotateCcw, Save } from "lucide-react";
import {
  memo,
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
import {
  type CompileDiagnostic,
  formatValidationError,
  type Lang,
  strings,
} from "@/i18n";
import {
  createWeftCompletionSource,
  findDeclarationRange,
} from "./weft-completion";
import { weftLanguage } from "./weft-language";

const PATH_SEP_RE = /[\\/]/;

const BASIC_SETUP = {
  lineNumbers: true,
  foldGutter: true,
  highlightActiveLine: true,
  highlightActiveLineGutter: true,
  bracketMatching: true,
  closeBrackets: true,
  autocompletion: false,
  indentOnInput: true,
  tabSize: 2,
};

function fileNameOf(filePath: string): string {
  const parts = filePath.split(PATH_SEP_RE);
  return parts[parts.length - 1] || filePath;
}

function toCodeMirrorDiagnostic(
  diag: CompileDiagnostic,
  view: EditorView,
  lang: Lang,
): Diagnostic {
  const docLen = view.state.doc.length;
  const message = formatValidationError(diag.error, lang);
  if (diag.span) {
    const from = Math.min(diag.span.start, docLen);
    let to = Math.min(docLen, Math.max(from, diag.span.end));
    if (from === to && from < docLen) to = from + 1;
    return { from, to, severity: "error", message };
  }
  return {
    from: 0,
    to: docLen > 0 ? view.state.doc.line(1).to : 0,
    severity: "error",
    message,
  };
}

const REVERT_ICON = <RotateCcw className="size-3.5" />;
const SAVE_ICON = <Save className="size-3.5" />;
const REFRESH_ICON = <RefreshCw className="size-3.5" />;

const WeftCodeMirror = memo(function WeftCodeMirror({
  draftStore,
  diagnostic,
  revealRequest,
  lang,
  onChange,
}: {
  draftStore: DraftStore;
  diagnostic: CompileDiagnostic | null;
  revealRequest: EditorRevealRequest | null;
  lang: Lang;
  onChange: (next: string) => void;
}) {
  const theme = useWorkspaceTheme();
  const value = useSyncExternalStore(
    draftStore.subscribe,
    draftStore.getSnapshot,
  );
  const langRef = useRef(lang);
  const [editorView, setEditorView] = useState<EditorView | null>(null);
  const handledRevealSeqRef = useRef(0);

  useEffect(() => {
    langRef.current = lang;
  }, [lang]);

  useEffect(() => {
    if (!editorView) return;
    const diagnostics = diagnostic
      ? [toCodeMirrorDiagnostic(diagnostic, editorView, lang)]
      : [];
    editorView.dispatch(setDiagnostics(editorView.state, diagnostics));
  }, [editorView, diagnostic, lang]);

  useEffect(() => {
    if (!editorView || !revealRequest) return;
    if (revealRequest.seq === handledRevealSeqRef.current) return;
    handledRevealSeqRef.current = revealRequest.seq;
    const range = findDeclarationRange(
      editorView.state.doc.toString(),
      revealRequest.kind,
      revealRequest.name,
    );
    if (!range) return;
    const docLen = editorView.state.doc.length;
    const from = Math.min(range.from, docLen);
    const to = Math.min(range.to, docLen);
    editorView.dispatch({
      selection: { anchor: from, head: to },
      effects: EditorView.scrollIntoView(from, { y: "center" }),
    });
    editorView.focus();
  }, [editorView, revealRequest]);

  const extensions = useMemo(
    () => [
      weftLanguage,
      autocompletion({
        override: [createWeftCompletionSource(() => langRef.current)],
        activateOnTyping: true,
        closeOnBlur: true,
        maxRenderedOptions: 30,
      }),
      lintGutter(),
      linter(null),
      EditorView.lineWrapping,
    ],
    [],
  );

  return (
    <CodeMirror
      className="h-full"
      value={value}
      height="100%"
      theme={theme}
      basicSetup={BASIC_SETUP}
      extensions={extensions}
      onChange={onChange}
      onCreateEditor={setEditorView}
    />
  );
});

function WeftEditorViewInner({
  path,
  draftStore,
  diagnostic,
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
  diagnostic: CompileDiagnostic | null;
  dirty: boolean;
  saving: boolean;
  diskStatus: DiskStatus;
  revealRequest: EditorRevealRequest | null;
  lang: Lang;
  onChange: (next: string) => void;
  onSave: () => Promise<boolean>;
  onForceSave: () => Promise<boolean>;
  onReloadFromDisk: () => Promise<void>;
}) {
  const t = strings[lang];
  const fileName = useMemo(() => fileNameOf(path), [path]);
  const canSave = (dirty || diskStatus === "deleted") && !saving;

  return (
    <div className="flex h-full min-h-0 flex-col bg-base-100">
      <div className="flex h-8 shrink-0 items-center gap-2 border-b border-base-300 bg-base-200 px-3">
        <span
          className="min-w-0 flex-1 truncate font-mono text-xs text-base-content/80"
          title={path}
        >
          {fileName}
        </span>
        {dirty && (
          <span className="badge badge-xs badge-outline shrink-0 border-primary text-primary">
            {t.unsaved}
          </span>
        )}
        <div className="flex shrink-0 items-center gap-1">
          {dirty && diskStatus !== "deleted" && (
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
          )}
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

      {diskStatus === "modified" && (
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
      )}

      {diskStatus === "deleted" && (
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
      )}

      <div className="min-h-0 flex-1 overflow-hidden">
        <WeftCodeMirror
          draftStore={draftStore}
          diagnostic={diagnostic}
          revealRequest={revealRequest}
          lang={lang}
          onChange={onChange}
        />
      </div>
    </div>
  );
}

export const WeftEditorView = memo(WeftEditorViewInner);
