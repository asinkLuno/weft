import "./App.css";

import { lazy, Suspense } from "react";
import { useDocument } from "@/features/document/use-document";
import { WelcomeView } from "@/features/welcome/welcome-view";
import { useAppSettings } from "@/hooks/use-app-settings";

const loadWorkspace = () => import("@/app/workspace");
const Workspace = lazy(() =>
  loadWorkspace().then((m) => ({ default: m.Workspace })),
);

function preloadWorkspace() {
  void loadWorkspace();
}

export default function App() {
  const { lang, recents, remember, forget } = useAppSettings();
  const {
    path,
    doc,
    draftStore,
    dirty,
    saving,
    hasExternalChange,
    flash,
    bind,
    updateDraft,
    save,
    reloadFromDisk,
    openFile,
    close,
  } = useDocument();

  async function openRecent(nextPath: string) {
    const result = await bind(nextPath);
    if (result === "bound") remember(nextPath);
    else if (result === "failed") forget(nextPath);
  }

  async function chooseFile() {
    const selected = await openFile();
    if (selected) await openRecent(selected);
  }

  return path ? (
    <Suspense fallback={null}>
      <Workspace
        key={path}
        path={path}
        doc={doc}
        draftStore={draftStore}
        dirty={dirty}
        saving={saving}
        hasExternalChange={hasExternalChange}
        lang={lang}
        flash={flash}
        onDraftChange={updateDraft}
        onSave={save}
        onReloadFromDisk={reloadFromDisk}
        onClose={close}
      />
    </Suspense>
  ) : (
    <WelcomeView
      lang={lang}
      recents={recents}
      onOpen={chooseFile}
      onRecent={openRecent}
      onPreload={preloadWorkspace}
    />
  );
}
