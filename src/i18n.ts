export type Lang = "zh" | "en";

// 文案多到难维护时再换 i18next，字典结构可以原样搬过去
export const strings = {
  zh: {
    tab1: "故事",
    tab2: "实体",
    tab3: "事件",
    tab4: "标签页 4",
    author: "作者",
    calendar: "历法",
    belongTo: "归属",
    linkGroup: "关系",
    baseTime: "基准时间",
    eventEmpty: "暂无事件",
    ganttEvent: "事件",
    ganttDuration: "持续时段",
    ganttMilestone: "时间点",
    noBaseTime: "无基准时间",
    open: "打开 YAML 文件",
    recent: "最近打开",
    deleted: "文件已删除或被移走",
    close: "关闭文件",
  },
  en: {
    tab1: "Story",
    tab2: "Entities",
    tab3: "Events",
    tab4: "Tab 4",
    author: "Author",
    calendar: "Calendar",
    belongTo: "Belongs to",
    linkGroup: "Links",
    baseTime: "Base time",
    eventEmpty: "No events",
    ganttEvent: "Event",
    ganttDuration: "Duration",
    ganttMilestone: "Milestone",
    noBaseTime: "No base time",
    open: "Open YAML File",
    recent: "Recent",
    deleted: "File deleted or moved",
    close: "Close File",
  },
} as const;

export function initialLang(): Lang {
  return navigator.language.startsWith("zh") ? "zh" : "en";
}

export function asLang(v: unknown): Lang | undefined {
  return v === "zh" || v === "en" ? v : undefined;
}
