import { LazyStore } from "@tauri-apps/plugin-store";

export const settings = new LazyStore("settings.json");

export const WORKSPACE_LAYOUT_KEY = "workspaceLayout:v1";

let preloadedWorkspaceLayout: Promise<unknown> | null = null;

export function preloadWorkspaceLayout(): Promise<unknown> {
  if (!preloadedWorkspaceLayout) {
    preloadedWorkspaceLayout = settings.get<unknown>(WORKSPACE_LAYOUT_KEY);
  }
  return preloadedWorkspaceLayout;
}

export function consumeWorkspaceLayout(): Promise<unknown> {
  const pending = preloadedWorkspaceLayout;
  preloadedWorkspaceLayout = null;
  return pending ?? settings.get<unknown>(WORKSPACE_LAYOUT_KEY);
}
