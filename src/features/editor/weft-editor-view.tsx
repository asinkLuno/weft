import { autocompletion } from "@codemirror/autocomplete";
import { EditorView } from "@codemirror/view";
import CodeMirror from "@uiw/react-codemirror";
import { RefreshCw, RotateCcw, Save } from "lucide-react";
import { memo, useEffect, useMemo, useRef, useSyncExternalStore } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { DraftStore } from "@/features/document/use-document";
import { type Lang, strings } from "@/i18n";
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

  useEffect(() => {
    langRef.current = lang;
  }, [lang]);

  const extensions = useMemo(
    () => [
      weftLanguage,
      autocompletion({
        override: [createWeftCompletionSource(() => langRef.current)],
        activateOnTyping: true,
        closeOnBlur: true,
        maxRenderedOptions: 30,
      }),
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
        />
      </div>
    </div>
  );
}

export const WeftEditorView = memo(WeftEditorViewInner);
