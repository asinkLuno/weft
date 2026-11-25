import { createContext, useContext } from "react";
import type { DiskStatus, DraftStore } from "@/features/document/use-document";
import type { CompileDiagnostic, Lang } from "@/i18n";
import type { WeftDoc } from "@/lib/document";

export type WorkspaceDocState = {
  path: string;
  doc: WeftDoc | null;
  lang: Lang;
};

export type WorkspaceEditorState = {
  draftStore: DraftStore;
  diagnostic: CompileDiagnostic | null;
  dirty: boolean;
  saving: boolean;
  diskStatus: DiskStatus;
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
