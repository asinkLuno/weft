import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { EditorView } from "@codemirror/view";
import { tags } from "@lezer/highlight";

export const weftEditorTheme = EditorView.theme({
  "&": {
    height: "100%",
    backgroundColor: "var(--color-base-100)",
    color: "var(--color-base-content)",
    fontSize: "13px",
    fontFamily:
      'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace',
  },
  "&.cm-focused": {
    outline: "none",
  },
  ".cm-scroller": {
    overflow: "auto",
    lineHeight: "1.65",
    fontFamily: "inherit",
  },
  ".cm-content": {
    padding: "12px 0",
    caretColor: "var(--color-primary)",
  },
  ".cm-line": {
    padding: "0 16px 0 12px",
  },
  ".cm-cursor, .cm-dropCursor": {
    borderLeftColor: "var(--color-primary)",
    borderLeftWidth: "2px",
  },
  "&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground":
    {
      backgroundColor:
        "color-mix(in srgb, var(--color-primary) 18%, transparent)",
    },
  ".cm-activeLine": {
    backgroundColor: "color-mix(in srgb, var(--color-primary) 5%, transparent)",
  },
  ".cm-gutters": {
    backgroundColor: "var(--color-base-200)",
    color: "color-mix(in oklab, var(--color-base-content) 55%, transparent)",
    borderRight: "1px solid var(--color-base-300)",
    userSelect: "none",
  },
  ".cm-lineNumbers .cm-gutterElement": {
    padding: "0 8px 0 12px",
    minWidth: "36px",
    fontSize: "12px",
  },
  ".cm-foldGutter .cm-gutterElement": {
    padding: "0 4px",
  },
  ".cm-activeLineGutter": {
    backgroundColor:
      "color-mix(in srgb, var(--color-primary) 10%, transparent)",
    color: "var(--color-primary)",
    fontWeight: "600",
  },
  ".cm-matchingBracket": {
    backgroundColor:
      "color-mix(in srgb, var(--color-primary) 18%, transparent)",
    outline:
      "1px solid color-mix(in srgb, var(--color-primary) 45%, transparent)",
  },
  ".cm-panels": {
    backgroundColor: "var(--color-base-200)",
    color: "var(--color-base-content)",
    borderColor: "var(--color-base-300)",
  },
  ".cm-panels.cm-panels-top": {
    borderBottom: "1px solid var(--color-base-300)",
  },
  ".cm-panels.cm-panels-bottom": {
    borderTop: "1px solid var(--color-base-300)",
  },
  ".cm-searchMatch": {
    backgroundColor: "color-mix(in srgb, var(--color-error) 20%, transparent)",
    outline:
      "1px solid color-mix(in srgb, var(--color-error) 50%, transparent)",
  },
  ".cm-searchMatch.cm-searchMatch-selected": {
    backgroundColor:
      "color-mix(in srgb, var(--color-primary) 26%, transparent)",
  },
  ".cm-textfield": {
    backgroundColor: "var(--color-base-100)",
    color: "var(--color-base-content)",
    border: "1px solid var(--color-base-300)",
    borderRadius: "var(--radius-field)",
  },
  ".cm-button": {
    backgroundImage: "none",
    backgroundColor: "var(--color-base-200)",
    color: "var(--color-base-content)",
    border: "1px solid var(--color-base-300)",
    borderRadius: "var(--radius-field)",
  },
  ".cm-tooltip": {
    backgroundColor: "var(--color-base-100)",
    color: "var(--color-base-content)",
    border: "1px solid var(--color-base-300)",
    borderRadius: "var(--radius-field)",
    boxShadow: "0 8px 24px -6px rgb(0 0 0 / 0.14)",
    overflow: "hidden",
  },
  ".cm-tooltip-autocomplete > ul": {
    fontFamily: "inherit",
    fontSize: "12.5px",
    maxHeight: "240px",
    padding: "4px",
  },
  ".cm-tooltip-autocomplete > ul > li": {
    display: "flex",
    alignItems: "center",
    gap: "8px",
    padding: "4px 8px",
    borderRadius: "calc(var(--radius-field) - 2px)",
  },
  ".cm-tooltip-autocomplete > ul > li[aria-selected]": {
    backgroundColor:
      "color-mix(in srgb, var(--color-primary) 14%, transparent)",
    color: "var(--color-base-content)",
  },
  ".cm-completionLabel": {
    color: "var(--color-base-content)",
  },
  ".cm-completionMatchedText": {
    color: "var(--color-primary)",
    fontWeight: "600",
    textDecoration: "none",
  },
  ".cm-completionDetail": {
    marginLeft: "auto",
    color: "color-mix(in oklab, var(--color-base-content) 55%, transparent)",
    fontSize: "11px",
    fontStyle: "normal",
  },
  ".cm-completionInfo": {
    backgroundColor: "var(--color-base-100)",
    color: "color-mix(in oklab, var(--color-base-content) 55%, transparent)",
    border: "1px solid var(--color-base-300)",
    borderRadius: "var(--radius-field)",
    padding: "6px 10px",
    fontSize: "12px",
    lineHeight: "1.5",
    maxWidth: "280px",
  },
  ".cm-lintRange-error": {
    backgroundImage: "none",
    textDecoration: "underline wavy var(--color-error)",
    textUnderlineOffset: "2px",
  },
  ".cm-tooltip-lint": {
    backgroundColor: "var(--color-base-100)",
    color: "var(--color-base-content)",
    border: "1px solid var(--color-base-300)",
    borderRadius: "var(--radius-field)",
    fontSize: "12px",
    lineHeight: "1.5",
    maxWidth: "360px",
  },
  ".cm-diagnostic-error": {
    borderLeft: "3px solid var(--color-error)",
    padding: "6px 10px",
  },
});

const weftHighlight = HighlightStyle.define([
  {
    tag: tags.propertyName,
    color: "var(--color-primary)",
    fontWeight: "500",
  },
  {
    tag: [tags.variableName, tags.string],
    color: "var(--color-base-content)",
  },
  { tag: tags.number, color: "var(--color-error)" },
  {
    tag: [tags.atom, tags.keyword],
    color: "var(--color-error)",
    fontWeight: "600",
  },
  {
    tag: tags.operator,
    color: "var(--color-primary)",
    fontWeight: "600",
  },
  {
    tag: tags.comment,
    color: "color-mix(in oklab, var(--color-base-content) 55%, transparent)",
    fontStyle: "italic",
  },
  {
    tag: tags.punctuation,
    color: "color-mix(in oklab, var(--color-base-content) 55%, transparent)",
  },
]);

export const weftThemeExtension = [
  weftEditorTheme,
  syntaxHighlighting(weftHighlight),
];
