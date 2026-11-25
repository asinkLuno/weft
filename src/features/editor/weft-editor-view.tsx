import { autocompletion } from "@codemirror/autocomplete";
import {
  type Diagnostic,
  linter,
  lintGutter,
  setDiagnostics,
} from "@codemirror/lint";
import { EditorView } from "@codemirror/view";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import CodeMirror from "@uiw/react-codemirror";
import { RefreshCw, RotateCcw, Save } from "lucide-react";
import {
  memo,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useSyncExternalStore,
} from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { DraftStore } from "@/features/document/use-document";
import {
  type CompileDiagnostic,
  formatValidationError,
  type Lang,
  strings,
} from "@/i18n";
import { createWeftCompletionSource } from "./weft-completion";
import { weftLanguage } from "./weft-language";
import { weftThemeExtension } from "./weft-theme";
import "./weft-editor.css";

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

async function checkDocument(
  view: EditorView,
  lang: Lang,
): Promise<Diagnostic[]> {
  try {
    const raw = await invoke<CompileDiagnostic | null>("check_document", {
      content: view.state.doc.toString(),
    });
    return raw ? [toCodeMirrorDiagnostic(raw, view, lang)] : [];
  } catch (error) {
    console.error("check_document failed:", error);
    return [];
  }
}

function WeftEditorViewInner({
  path,
  draftStore,
  dirty,
  saving,
  hasExternalChange,
  lang,
  onChange,
  onSave,
  onReloadFromDisk,
}: {
  path: string;
  draftStore: DraftStore;
  dirty: boolean;
  saving: boolean;
  hasExternalChange: boolean;
  lang: Lang;
  onChange: (next: string) => void;
  onSave: () => Promise<boolean>;
  onReloadFromDisk: () => void;
}) {
  const value = useSyncExternalStore(
    draftStore.subscribe,
    draftStore.getSnapshot,
  );
  const t = strings[lang];
  const langRef = useRef(lang);
  const viewRef = useRef<EditorView | null>(null);
  const refreshSeqRef = useRef(0);

  const refreshDiagnostics = useCallback(() => {
    const view = viewRef.current;
    if (!view) return;
    const doc = view.state.doc;
    const seq = ++refreshSeqRef.current;
    void checkDocument(view, langRef.current).then((diagnostics) => {
      if (
        viewRef.current === view &&
        refreshSeqRef.current === seq &&
        view.state.doc === doc
      ) {
        view.dispatch(setDiagnostics(view.state, diagnostics));
      }
    });
  }, []);

  useEffect(() => {
    if (langRef.current === lang) return;
    langRef.current = lang;
    refreshDiagnostics();
  }, [lang, refreshDiagnostics]);

  // 外部文件或依赖脚本变更时刷新诊断（.weft 文本未变时 CodeMirror linter 不会自动重跑）
  useEffect(() => {
    let active = true;
    let unlisten: UnlistenFn | undefined;
    void listen("document-changed", () => {
      if (active) refreshDiagnostics();
    })
      .then((fn) => {
        if (active) unlisten = fn;
        else fn();
      })
      .catch((error: unknown) => {
        console.error("editor lint listener failed:", error);
      });
    return () => {
      active = false;
      viewRef.current = null;
      unlisten?.();
    };
  }, [refreshDiagnostics]);

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
      linter((view) => checkDocument(view, langRef.current), { delay: 250 }),
      EditorView.lineWrapping,
      ...weftThemeExtension,
    ],
    [],
  );

  return (
    <div className="weft-editor-view">
      <div className="weft-editor-toolbar">
        <span className="weft-editor-filename" title={path}>
          {fileNameOf(path)}
        </span>
        {dirty && (
          <Badge variant="secondary" className="px-1.5 py-0 text-[11px]">
            {t.unsaved}
          </Badge>
        )}
        <div className="weft-editor-actions">
          {dirty && (
            <Button
              variant="ghost"
              size="xs"
              disabled={saving}
              onClick={onReloadFromDisk}
            >
              <RotateCcw className="size-3.5" />
              {t.revert}
            </Button>
          )}
          <Button
            variant="default"
            size="xs"
            disabled={!dirty || saving}
            onClick={() => {
              void onSave();
            }}
          >
            <Save className="size-3.5" />
            {saving ? t.saving : t.save}
          </Button>
        </div>
      </div>

      {hasExternalChange && (
        <div className="weft-editor-banner">
          <span>{t.externalChanged}</span>
          <Button variant="outline" size="xs" onClick={onReloadFromDisk}>
            <RefreshCw className="size-3" />
            {t.reloadFromDisk}
          </Button>
        </div>
      )}

      <div className="weft-editor-surface">
        <CodeMirror
          className="weft-editor-cm"
          value={value}
          height="100%"
          theme="none"
          basicSetup={BASIC_SETUP}
          extensions={extensions}
          onChange={onChange}
          onCreateEditor={(view) => {
            viewRef.current = view;
          }}
        />
      </div>
    </div>
  );
}

export const WeftEditorView = memo(WeftEditorViewInner);
