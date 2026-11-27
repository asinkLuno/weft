export type ThemeMode = "system" | "light" | "dark";
export type ResolvedTheme = "light" | "dark";

const DAISY_THEME: Record<ResolvedTheme, string> = {
  light: "lofi",
  dark: "dark",
};

const DARK_MEDIA_QUERY = "(prefers-color-scheme: dark)";
const NOOP_UNSUBSCRIBE = () => {};
let darkColorSchemeQuery: MediaQueryList | null | undefined;

function getDarkColorSchemeQuery(): MediaQueryList | null {
  if (darkColorSchemeQuery !== undefined) return darkColorSchemeQuery;
  darkColorSchemeQuery =
    typeof window !== "undefined" && typeof window.matchMedia === "function"
      ? window.matchMedia(DARK_MEDIA_QUERY)
      : null;
  return darkColorSchemeQuery;
}

export function asThemeMode(value: unknown): ThemeMode | undefined {
  return value === "system" || value === "light" || value === "dark"
    ? value
    : undefined;
}

export function resolveTheme(
  mode: ThemeMode,
  systemDark: boolean,
): ResolvedTheme {
  if (mode === "system") return systemDark ? "dark" : "light";
  return mode;
}

export function subscribeSystemDark(onStoreChange: () => void): () => void {
  const mediaQuery = getDarkColorSchemeQuery();
  if (!mediaQuery) return NOOP_UNSUBSCRIBE;
  mediaQuery.addEventListener("change", onStoreChange);
  return () => {
    mediaQuery.removeEventListener("change", onStoreChange);
  };
}

export function subscribeNoop(): () => void {
  return NOOP_UNSUBSCRIBE;
}

export function getSystemDarkSnapshot(): boolean {
  return getDarkColorSchemeQuery()?.matches ?? false;
}

export function getFalseSnapshot(): boolean {
  return false;
}

export function applyDocumentTheme(theme: ResolvedTheme): void {
  document.documentElement.dataset.theme = DAISY_THEME[theme];
}
