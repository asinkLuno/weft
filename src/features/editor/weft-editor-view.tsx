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
import type { DiskStatus, DraftStore } from "@/features/document/use-document";
import {
  type CompileDiagnostic,
  formatValidationError,
  type Lang,
  strings,
} from "@/i18n";
import { createWeftCompletionSource } from "./weft-completion";
import { weftLanguage } from "./weft-language";
import { weftThemeExtension } from "./weft-theme";

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

function WeftEditorViewInner({
  path,
  draftStore,
  diagnostic,
  dirty,
  saving,
  diskStatus,
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
  lang: Lang;
  onChange: (next: string) => void;
  onSave: () => Promise<boolean>;
  onForceSave: () => Promise<boolean>;
  onReloadFromDisk: () => Promise<void>;
}) {
  const value = useSyncExternalStore(
    draftStore.subscribe,
    draftStore.getSnapshot,
  );
  const t = strings[lang];
  const langRef = useRef(lang);
  langRef.current = lang;
  const [editorView, setEditorView] = useState<EditorView | null>(null);

  useEffect(() => {
    if (!editorView) return;
    const diagnostics = diagnostic
      ? [toCodeMirrorDiagnostic(diagnostic, editorView, lang)]
      : [];
    editorView.dispatch(setDiagnostics(editorView.state, diagnostics));
  }, [editorView, diagnostic, lang]);

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
      ...weftThemeExtension,
    ],
    [],
  );

  const canSave = (dirty || diskStatus === "deleted") && !saving;

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex items-center gap-2 border-b border-base-300 bg-base-200 px-3 py-1.5">
        <span
          className="min-w-0 flex-1 truncate font-mono text-xs"
          title={path}
        >
          {fileNameOf(path)}
        </span>
        {dirty && (
          <span className="badge badge-neutral badge-sm shrink-0">
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
              <RotateCcw className="size-3.5" />
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
            <Save className="size-3.5" />
            {saving ? t.saving : t.save}
          </button>
        </div>
      </div>

      {diskStatus === "modified" && (
        <div className="flex flex-wrap items-center justify-between gap-2 border-b border-warning bg-warning/15 px-3 py-2 text-xs">
          <span>{t.externalChanged}</span>
          <div className="flex items-center gap-1">
            <button
              type="button"
              className="btn btn-outline btn-xs"
              disabled={saving}
              onClick={() => {
                void onReloadFromDisk();
              }}
            >
              <RefreshCw className="size-3" />
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
              <Save className="size-3" />
              {t.overwriteDisk}
            </button>
          </div>
        </div>
      )}

      {diskStatus === "deleted" && (
        <div className="flex flex-wrap items-center justify-between gap-2 border-b border-error bg-error/15 px-3 py-2 text-xs">
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
              <Save className="size-3" />
              {t.recreateFile}
            </button>
          </div>
        </div>
      )}

      <div className="min-h-0 flex-1 overflow-hidden">
        <CodeMirror
          className="h-full"
          value={value}
          height="100%"
          theme="none"
          basicSetup={BASIC_SETUP}
          extensions={extensions}
          onChange={onChange}
          onCreateEditor={(view) => {
            setEditorView(view);
          }}
        />
      </div>
    </div>
  );
}

export const WeftEditorView = memo(WeftEditorViewInner);
