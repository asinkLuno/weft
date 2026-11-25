import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { EditorView } from "@codemirror/view";
import { tags } from "@lezer/highlight";

export const weftEditorTheme = EditorView.theme({
  "&": {
    height: "100%",
    backgroundColor: "var(--card)",
    color: "var(--foreground)",
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
    caretColor: "var(--riso-blue)",
  },
  ".cm-line": {
    padding: "0 16px 0 12px",
  },
  ".cm-cursor, .cm-dropCursor": {
    borderLeftColor: "var(--riso-blue)",
    borderLeftWidth: "2px",
  },
  "&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground":
    {
      backgroundColor: "color-mix(in srgb, var(--riso-blue) 18%, transparent)",
    },
  ".cm-activeLine": {
    backgroundColor: "color-mix(in srgb, var(--riso-blue) 5%, transparent)",
  },
  ".cm-gutters": {
    backgroundColor: "var(--background)",
    color: "var(--muted-foreground)",
    borderRight: "1px solid var(--border)",
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
    backgroundColor: "color-mix(in srgb, var(--riso-blue) 10%, transparent)",
    color: "var(--riso-blue)",
    fontWeight: "600",
  },
  ".cm-matchingBracket": {
    backgroundColor: "color-mix(in srgb, var(--riso-blue) 18%, transparent)",
    outline: "1px solid color-mix(in srgb, var(--riso-blue) 45%, transparent)",
  },
  ".cm-panels": {
    backgroundColor: "var(--background)",
    color: "var(--foreground)",
    borderColor: "var(--border)",
  },
  ".cm-panels.cm-panels-top": {
    borderBottom: "1px solid var(--border)",
  },
  ".cm-panels.cm-panels-bottom": {
    borderTop: "1px solid var(--border)",
  },
  ".cm-searchMatch": {
    backgroundColor: "color-mix(in srgb, var(--riso-red) 20%, transparent)",
    outline: "1px solid color-mix(in srgb, var(--riso-red) 50%, transparent)",
  },
  ".cm-searchMatch.cm-searchMatch-selected": {
    backgroundColor: "color-mix(in srgb, var(--riso-blue) 26%, transparent)",
  },
  ".cm-textfield": {
    backgroundColor: "var(--card)",
    color: "var(--foreground)",
    border: "1px solid var(--border)",
    borderRadius: "var(--radius)",
  },
  ".cm-button": {
    backgroundImage: "none",
    backgroundColor: "var(--secondary)",
    color: "var(--secondary-foreground)",
    border: "1px solid var(--border)",
    borderRadius: "var(--radius)",
  },
});

const weftHighlight = HighlightStyle.define([
  {
    tag: [tags.propertyName, tags.attributeName, tags.name],
    color: "var(--riso-blue)",
    fontWeight: "500",
  },
  { tag: [tags.string, tags.special(tags.string)], color: "var(--foreground)" },
  { tag: [tags.number, tags.integer, tags.float], color: "var(--riso-red)" },
  {
    tag: [tags.bool, tags.null, tags.atom, tags.keyword],
    color: "var(--riso-red)",
    fontWeight: "600",
  },
  {
    tag: [tags.comment, tags.lineComment, tags.blockComment],
    color: "var(--muted-foreground)",
    fontStyle: "italic",
  },
  {
    tag: [
      tags.punctuation,
      tags.separator,
      tags.bracket,
      tags.squareBracket,
      tags.brace,
    ],
    color: "var(--muted-foreground)",
  },
  { tag: [tags.meta, tags.labelName], color: "var(--riso-blue)" },
  {
    tag: tags.invalid,
    color: "var(--destructive)",
    textDecoration: "underline wavy",
  },
]);

export const weftYamlThemeExtension = [
  weftEditorTheme,
  syntaxHighlighting(weftHighlight),
];
