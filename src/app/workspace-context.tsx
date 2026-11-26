import { createContext, useContext } from "react";
import type { DiskStatus, DraftStore } from "@/features/document/use-document";
import type { WeftDeclKind } from "@/features/editor/weft-completion";
import type { CompileDiagnostic, Lang } from "@/i18n";
import type { WeftDoc } from "@/lib/document";

export type EditorRevealRequest = {
  kind: WeftDeclKind;
  name: string;
  seq: number;
};

export type WorkspaceDocState = {
  path: string;
  doc: WeftDoc | null;
  lang: Lang;
  revealInEditor: (kind: WeftDeclKind, name: string) => void;
};

export type WorkspaceEditorState = {
  draftStore: DraftStore;
  diagnostic: CompileDiagnostic | null;
  dirty: boolean;
  saving: boolean;
  diskStatus: DiskStatus;
  revealRequest: EditorRevealRequest | null;
  onDraftChange: (next: string) => void;
  onSave: () => Promise<boolean>;
  onForceSave: () => Promise<boolean>;
  onReloadFromDisk: () => Promise<void>;
};

export const WorkspaceDocContext = createContext<WorkspaceDocState | null>(
  null,
);
export const WorkspaceEditorContext =
  createContext<WorkspaceEditorState | null>(null);

export function useWorkspaceDoc(): WorkspaceDocState {
  const ctx = useContext(WorkspaceDocContext);
  if (!ctx) {
    throw new Error("useWorkspaceDoc must be used inside Workspace");
  }
  return ctx;
}

export function useWorkspaceEditor(): WorkspaceEditorState {
  const ctx = useContext(WorkspaceEditorContext);
  if (!ctx) {
    throw new Error("useWorkspaceEditor must be used inside Workspace");
  }
  return ctx;
}
