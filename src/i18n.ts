export type Lang = "zh" | "en";

// 文案多到难维护时再换 i18next，字典结构可以原样搬过去
export const strings = {
  zh: {
    parseView: "解析视图",
  },
  en: {
    parseView: "Parse View",
  },
} as const;

export function initialLang(): Lang {
  return navigator.language.startsWith("zh") ? "zh" : "en";
}

export function asLang(v: unknown): Lang | undefined {
  return v === "zh" || v === "en" ? v : undefined;
}
