import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import { asLang, initialLang, type Lang } from "@/i18n";
import { settings } from "@/lib/settings";

export function useAppSettings() {
  const [lang, setLang] = useState<Lang>(initialLang);
  const [recents, setRecents] = useState<string[]>([]);
  const recentFiles = useRef<string[]>([]);
  const recentsChanged = useRef(false);

  useEffect(() => {
    let cancelled = false;
    let languageChanged = false;
    const subscription = listen<{ language?: Lang }>(
      "settings-changed",
      ({ payload }) => {
        if (cancelled) return;
        const next = asLang(payload.language);
        if (next) {
          languageChanged = true;
          setLang(next);
        }
      },
    );
    void subscription
      .then(() => invoke<unknown>("get_language"))
      .then((value) => {
        if (cancelled || languageChanged) return;
        const next = asLang(value);
        if (next) setLang(next);
      })
      .catch(console.error);
    void settings
      .get<unknown>("recentFiles")
      .then((value) => {
        if (cancelled || recentsChanged.current || !Array.isArray(value))
          return;
        const next = value.filter(
          (item): item is string => typeof item === "string",
        );
        recentFiles.current = next;
        setRecents(next);
      })
      .catch(console.error);
    return () => {
      cancelled = true;
      void subscription.then((unlisten) => unlisten()).catch(console.error);
    };
  }, []);

  function updateRecents(next: string[]) {
    recentsChanged.current = true;
    recentFiles.current = next;
    setRecents(next);
    void settings.set("recentFiles", next).catch(console.error);
  }

  function remember(path: string) {
    updateRecents(
      [path, ...recentFiles.current.filter((item) => item !== path)].slice(
        0,
        5,
      ),
    );
  }

  function forget(path: string) {
    updateRecents(recentFiles.current.filter((item) => item !== path));
  }

  return { lang, recents, remember, forget };
}
