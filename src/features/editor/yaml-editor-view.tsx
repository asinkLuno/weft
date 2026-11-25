import { yaml } from "@codemirror/lang-yaml";
import { EditorView, keymap } from "@codemirror/view";
import CodeMirror from "@uiw/react-codemirror";
import { RefreshCw, RotateCcw, Save } from "lucide-react";
import { memo, useEffect, useMemo, useRef, useSyncExternalStore } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { DraftStore } from "@/features/document/use-document";
import { type Lang, strings } from "@/i18n";
import { weftYamlThemeExtension } from "./yaml-theme";
import "./yaml-editor.css";

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

function YamlEditorViewInner({
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
  const onSaveRef = useRef(onSave);
  const dirtyRef = useRef(dirty);

  useEffect(() => {
    onSaveRef.current = onSave;
  }, [onSave]);

  useEffect(() => {
    dirtyRef.current = dirty;
  }, [dirty]);

  const extensions = useMemo(
    () => [
      yaml(),
      EditorView.lineWrapping,
      keymap.of([
        {
          key: "Mod-s",
          preventDefault: true,
          run: () => {
            if (dirtyRef.current) {
              void onSaveRef.current();
            }
            return true;
          },
        },
      ]),
      ...weftYamlThemeExtension,
    ],
    [],
  );

  return (
    <div className="yaml-editor-view">
      <div className="yaml-editor-toolbar">
        <span className="yaml-editor-filename" title={path}>
          {fileNameOf(path)}
        </span>
        {dirty && (
          <Badge variant="secondary" className="px-1.5 py-0 text-[11px]">
            {t.unsaved}
          </Badge>
        )}
        <div className="yaml-editor-actions">
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
        <div className="yaml-editor-banner">
          <span>{t.externalChanged}</span>
          <Button variant="outline" size="xs" onClick={onReloadFromDisk}>
            <RefreshCw className="size-3" />
            {t.reloadFromDisk}
          </Button>
        </div>
      )}

      <div className="yaml-editor-surface">
        <CodeMirror
          className="yaml-editor-cm"
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

export const YamlEditorView = memo(YamlEditorViewInner);
