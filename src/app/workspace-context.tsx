import { createContext, useContext } from "react";
import type { DiskStatus, DraftStore } from "@/features/document/use-document";
import type { WeftDeclKind } from "@/features/editor/weft-ls-client";
import type { Lang } from "@/i18n";
import type { WeftDoc } from "@/lib/document";
import type { ResolvedTheme } from "@/lib/theme";
import type { DocumentTone } from "@/lib/ui";

export type EditorRevealRequest = {
  kind: WeftDeclKind;
  name: string;
  seq: number;
};

export type WorkspaceMetaState = {
  path: string;
  lang: Lang;
  revealInEditor: (kind: WeftDeclKind, name: string) => void;
};

export type WorkspaceEditorState = {
  draftStore: DraftStore;
  dirty: boolean;
  saving: boolean;
  diskStatus: DiskStatus;
  revealRequest: EditorRevealRequest | null;
  onDraftChange: (docLength: number, read: () => string) => void;
  onSave: () => Promise<boolean>;
  onForceSave: () => Promise<boolean>;
  onReloadFromDisk: () => Promise<void>;
};

export const WorkspaceMetaContext = createContext<WorkspaceMetaState | null>(
  null,
);
export const WorkspaceDocContext = createContext<WeftDoc | null>(null);
export const WorkspaceEditorContext =
  createContext<WorkspaceEditorState | null>(null);
export const WorkspaceToneContext = createContext<DocumentTone>("synced");
export const WorkspaceThemeContext = createContext<ResolvedTheme>("light");

export function useWorkspaceMeta(): WorkspaceMetaState {
  const ctx = useContext(WorkspaceMetaContext);
  if (!ctx) {
    throw new Error("useWorkspaceMeta must be used inside Workspace");
  }
  return ctx;
}

export function useWorkspaceDoc(): WeftDoc | null {
  return useContext(WorkspaceDocContext);
}

export function useWorkspaceEditor(): WorkspaceEditorState {
  const ctx = useContext(WorkspaceEditorContext);
  if (!ctx) {
    throw new Error("useWorkspaceEditor must be used inside Workspace");
  }
  return ctx;
}

export function useWorkspaceTone(): DocumentTone {
  return useContext(WorkspaceToneContext);
}

export function useWorkspaceTheme(): ResolvedTheme {
  return useContext(WorkspaceThemeContext);
}
