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

type AppSettingsPayload = {
  language?: unknown;
  theme?: unknown;
  formatOnSave?: unknown;
};

export function useAppSettings() {
  const [lang, setLang] = useState<Lang>(initialLang);
  const [themeMode, setThemeMode] = useState<ThemeMode>("system");
  const [formatOnSave, setFormatOnSave] = useState(false);
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
    let formatOnSaveChanged = false;
    const subscription = listen<AppSettingsPayload>(
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
        if (typeof payload.formatOnSave === "boolean") {
          formatOnSaveChanged = true;
          setFormatOnSave(payload.formatOnSave);
        }
      },
    );

    // 合并启动 IPC 与本地存储读取，在同一次微任务里批量更新状态（避免 4 次连续根重渲染）
    void Promise.allSettled([
      invoke<AppSettingsPayload>("get_app_settings"),
      settings.get<unknown>("recentFiles"),
    ]).then(([settingsRes, recentsRes]) => {
      if (cancelled) return;
      if (settingsRes.status === "fulfilled") {
        const appSettings = settingsRes.value;
        if (!languageChanged) {
          const next = asLang(appSettings.language);
          if (next) setLang(next);
        }
        if (!themeChanged) {
          const next = asThemeMode(appSettings.theme);
          if (next) setThemeMode(next);
        }
        if (
          !formatOnSaveChanged &&
          typeof appSettings.formatOnSave === "boolean"
        ) {
          setFormatOnSave(appSettings.formatOnSave);
        }
      } else {
        console.error("get_app_settings failed:", settingsRes.reason);
      }

      if (recentsRes.status === "fulfilled") {
        const value = recentsRes.value;
        if (!recentsChanged.current && Array.isArray(value)) {
          const next = value.filter(
            (item): item is string => typeof item === "string",
          );
          recentFiles.current = next;
          setRecents(next);
        }
      } else {
        console.error("recentFiles load failed:", recentsRes.reason);
      }
    });

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

  return { lang, theme, formatOnSave, recents, remember, forget };
}
