import { lazy, Suspense, useCallback } from "react";
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
  const { lang, theme, recents, remember, forget } = useAppSettings();
  const {
    path,
    doc,
    diagnostic,
    draftStore,
    dirty,
    saving,
    diskStatus,
    flash,
    bind,
    updateDraft,
    save,
    forceSave,
    reloadFromDisk,
    openFile,
    close,
  } = useDocument(lang);

  const openRecent = useCallback(
    async (nextPath: string) => {
      const result = await bind(nextPath);
      if (result === "bound") remember(nextPath);
      else if (result === "failed") forget(nextPath);
    },
    [bind, forget, remember],
  );

  const chooseFile = useCallback(async () => {
    const selected = await openFile();
    if (selected) await openRecent(selected);
  }, [openFile, openRecent]);

  return path ? (
    <Suspense fallback={null}>
      <Workspace
        key={path}
        path={path}
        doc={doc}
        diagnostic={diagnostic}
        draftStore={draftStore}
        dirty={dirty}
        saving={saving}
        diskStatus={diskStatus}
        lang={lang}
        theme={theme}
        flash={flash}
        onDraftChange={updateDraft}
        onSave={save}
        onForceSave={forceSave}
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
