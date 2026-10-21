export type Lang = "zh" | "en";

// 文案多到难维护时再换 i18next，字典结构可以原样搬过去
export const strings = {
  zh: {
    parseView: "解析视图",
    open: "打开 YAML 文件",
    recent: "最近打开",
  },
  en: {
    parseView: "Parse View",
    open: "Open YAML File",
    recent: "Recent",
  },
} as const;

export function initialLang(): Lang {
  return navigator.language.startsWith("zh") ? "zh" : "en";
}

export function asLang(v: unknown): Lang | undefined {
  return v === "zh" || v === "en" ? v : undefined;
}
