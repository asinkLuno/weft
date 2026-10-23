import "./App.css";

import { DocumentErrorToast } from "@/app/document-error-toast";
import { Workspace } from "@/app/workspace";
import { useDocument } from "@/features/document/use-document";
import { WelcomeView } from "@/features/welcome/welcome-view";
import { useAppSettings } from "@/hooks/use-app-settings";

export default function App() {
  const { lang, recents, remember, forget } = useAppSettings();
  const { path, doc, activeError, flash, bind, openFile, close } =
    useDocument();

  async function openRecent(nextPath: string) {
    const result = await bind(nextPath);
    if (result === "bound") remember(nextPath);
    else if (result === "failed") forget(nextPath);
  }

  async function chooseFile() {
    const selected = await openFile();
    if (selected) await openRecent(selected);
  }

  return (
    <>
      <DocumentErrorToast error={activeError} lang={lang} />
      {path ? (
        <Workspace
          key={path}
          path={path}
          doc={doc}
          lang={lang}
          flash={flash}
          activeError={activeError}
          onClose={close}
        />
      ) : (
        <WelcomeView
          lang={lang}
          recents={recents}
          onOpen={chooseFile}
          onRecent={openRecent}
        />
      )}
    </>
  );
}
