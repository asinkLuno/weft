import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import { asLang, initialLang, type Lang } from "@/i18n";
import { settings } from "@/lib/settings";
import {
  applyDocumentTheme,
  asThemeMode,
  getFalseSnapshot,
  getSystemDarkSnapshot,
  resolveTheme,
  subscribeNoop,
  subscribeSystemDark,
  type ThemeMode,
} from "@/lib/theme";

export function useAppSettings() {
  const [lang, setLang] = useState<Lang>(initialLang);
  const [themeMode, setThemeMode] = useState<ThemeMode>("system");
  const followSystem = themeMode === "system";
  // 仅在跟随系统时订阅 prefers-color-scheme，固定浅色/深色时系统切换不触发重渲染
  const systemDark = useSyncExternalStore(
    followSystem ? subscribeSystemDark : subscribeNoop,
    followSystem ? getSystemDarkSnapshot : getFalseSnapshot,
  );
  const [recents, setRecents] = useState<string[]>([]);
  const recentFiles = useRef<string[]>([]);
  const recentsChanged = useRef(false);

  const theme = resolveTheme(themeMode, systemDark);

  useLayoutEffect(() => {
    applyDocumentTheme(theme);
  }, [theme]);

  useEffect(() => {
    let cancelled = false;
    let languageChanged = false;
    let themeChanged = false;
    const subscription = listen<{ language?: unknown; theme?: unknown }>(
      "settings-changed",
      ({ payload }) => {
        if (cancelled) return;
        const nextLang = asLang(payload.language);
        if (nextLang) {
          languageChanged = true;
          setLang(nextLang);
        }
        const nextTheme = asThemeMode(payload.theme);
        if (nextTheme) {
          themeChanged = true;
          setThemeMode(nextTheme);
        }
      },
    );
    // 与 listen 并行：*Changed 已经挡住"事件先到、旧值后到"的覆盖
    void invoke<unknown>("get_language")
      .then((value) => {
        if (cancelled || languageChanged) return;
        const next = asLang(value);
        if (next) setLang(next);
      })
      .catch(console.error);
    void invoke<unknown>("get_theme")
      .then((value) => {
        if (cancelled || themeChanged) return;
        const next = asThemeMode(value);
        if (next) setThemeMode(next);
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

  const updateRecents = useCallback((next: string[]) => {
    recentsChanged.current = true;
    recentFiles.current = next;
    setRecents(next);
    void settings.set("recentFiles", next).catch(console.error);
  }, []);

  const remember = useCallback(
    (path: string) => {
      updateRecents(
        [path, ...recentFiles.current.filter((item) => item !== path)].slice(
          0,
          5,
        ),
      );
    },
    [updateRecents],
  );

  const forget = useCallback(
    (path: string) => {
      updateRecents(recentFiles.current.filter((item) => item !== path));
    },
    [updateRecents],
  );

  return { lang, theme, recents, remember, forget };
}
