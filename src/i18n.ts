import type { ValidationError } from "./generated/validation-error";

export type Lang = "zh" | "en";

export type DocumentError =
  | { type: "validation"; error: ValidationError }
  | { type: "references"; errors: ValidationError[] }
  | { type: "raw"; message: string };

type ValidationCode = ValidationError["code"];

const validationMessages = {
  zh: {
    file_deleted: "文件已删除或被移走",
    document_not_bound: "尚未绑定文档",
    invalid_path: "无效路径",
    duplicate_event_title: "事件标题 `{title}` 在分组 {groups} 中重复",
    unknown_link_entity: "entity_link `{group}` 引用了不存在的实体 `{entity}`",
    unknown_event_entity: "event `{event}` 引用了不存在的实体 `{entity}`",
    unknown_observer: "narrative `{narrative}` 引用了不存在的实体 `{entity}`",
    unknown_subject_event:
      "narrative `{narrative}` 的 subject 引用了不存在的事件 `{event}`",
    entity_unknown_parent:
      "entity `{entity}` belong_to 引用了不存在的实体 `{target}`",
    entity_cycle: "entity `{entity}` 的 belong_to 形成了循环",
    time_nonempty_list: "时间必须是非空列表",
    time_integer_component: "时间分量必须是整数",
    event_end_before_start: "事件 `{event}` 的结束时间早于开始时间",
    unregistered_calendar:
      "未注册的历法 `{calendar}`，请在顶层 calendar 中注册",
    invalid_plugin_spec: "`{name}` 需要 runtime: rhai, kind: {kind}, api: 1",
    calendar_components_not_positive_integer:
      "历法 {calendar} 的 metadata.components 必须是正整数",
    calendar_components_out_of_range:
      "历法 {calendar} 的 metadata.components 必须在 1 到 64 之间",
    calendar_file_read: "读取历法 `{calendar}` 的脚本失败：{source}",
    calendar_compile: "历法 `{calendar}` 编译失败：{source}",
    calendar_call: "历法 `{calendar}` 的 `{entry}` 执行失败：{source}",
    calendar_invalid_metadata:
      "历法 `{calendar}` 返回的 metadata 无效：{source}",
    calendar_invalid_return: "历法 `{calendar}` 的 `{entry}` 返回值类型无效",
    too_many_time_components:
      "时间分量过多：{actual} 个，该历法最多 {maximum} 个",
    normalize_wrong_component_count: "normalize 必须返回 {expected} 个时间分量",
    time_offset_out_of_range: "时间偏移超出整数范围",
  },
  en: {
    file_deleted: "File deleted or moved",
    document_not_bound: "No document is bound",
    invalid_path: "Invalid path",
    duplicate_event_title: "Duplicate event title `{title}` in groups {groups}",
    unknown_link_entity:
      "entity_link `{group}` refers to unknown entity `{entity}`",
    unknown_event_entity: "event `{event}` refers to unknown entity `{entity}`",
    unknown_observer:
      "narrative `{narrative}` refers to unknown entity `{entity}`",
    unknown_subject_event:
      "subject in narrative `{narrative}` refers to unknown event `{event}`",
    entity_unknown_parent:
      "entity `{entity}` belongs to unknown entity `{target}`",
    entity_cycle: "belong_to for entity `{entity}` forms a cycle",
    time_nonempty_list: "Time must be a nonempty list",
    time_integer_component: "Time components must be integers",
    event_end_before_start: "End time is before start time for event `{event}`",
    unregistered_calendar:
      "Calendar `{calendar}` is not registered in the top-level calendar section",
    invalid_plugin_spec:
      "`{name}` requires runtime: rhai, kind: {kind}, api: 1",
    calendar_components_not_positive_integer:
      "metadata.components for calendar {calendar} must be a positive integer",
    calendar_components_out_of_range:
      "metadata.components for calendar {calendar} must be between 1 and 64",
    calendar_file_read:
      "Failed to read the script of calendar `{calendar}`: {source}",
    calendar_compile: "Failed to compile calendar `{calendar}`: {source}",
    calendar_call: "Error running `{entry}` of calendar `{calendar}`: {source}",
    calendar_invalid_metadata:
      "Invalid metadata returned by calendar `{calendar}`: {source}",
    calendar_invalid_return:
      "`{entry}` of calendar `{calendar}` returned an invalid value",
    too_many_time_components:
      "Too many time components: {actual}; this calendar allows at most {maximum}",
    normalize_wrong_component_count:
      "normalize must return {expected} time components",
    time_offset_out_of_range: "Time offset is outside the integer range",
  },
} satisfies Record<Lang, Record<ValidationCode, string>>;

function formatValidationError(error: ValidationError, lang: Lang): string {
  const code = error.code;
  const messages: Record<string, string> = validationMessages[lang];
  const template = messages[code] ?? code;
  const params = ("params" in error ? error.params : {}) as Record<
    string,
    string
  >;
  return template.replace(
    /\{(\w+)\}/g,
    (_, key: string) => params[key] ?? `{${key}}`,
  );
}

export function formatDocumentError(error: unknown, lang: Lang): string {
  if (error && typeof error === "object" && "type" in error) {
    const e = error as Partial<DocumentError>;
    if (e.type === "raw" && typeof e.message === "string") return e.message;
    if (e.type === "references" && Array.isArray(e.errors)) {
      return e.errors
        .map((item) => `• ${formatValidationError(item, lang)}`)
        .join("\n");
    }
    if (e.type === "validation" && e.error) {
      return formatValidationError(e.error, lang);
    }
  }
  return String(error);
}

// 文案多到难维护时再换 i18next，字典结构可以原样搬过去
export const strings = {
  zh: {
    tab1: "故事",
    tab2: "实体",
    tab3: "事件",
    tab4: "叙事",
    author: "作者",
    calendar: "历法",
    belongTo: "归属",
    linkGroup: "关系",
    baseTime: "基准时间",
    eventEmpty: "暂无事件",
    narrativeEmpty: "暂无叙事",
    observer: "观察者",
    ganttEvent: "事件",
    ganttDuration: "持续时段",
    ganttMilestone: "时间点",
    noBaseTime: "无基准时间",
    open: "打开 YAML 文件",
    recent: "最近打开",
    close: "关闭文件",
  },
  en: {
    tab1: "Story",
    tab2: "Entities",
    tab3: "Events",
    tab4: "Narratives",
    author: "Author",
    calendar: "Calendar",
    belongTo: "Belongs to",
    linkGroup: "Links",
    baseTime: "Base time",
    eventEmpty: "No events",
    narrativeEmpty: "No narratives",
    observer: "Observer",
    ganttEvent: "Event",
    ganttDuration: "Duration",
    ganttMilestone: "Milestone",
    noBaseTime: "No base time",
    open: "Open YAML File",
    recent: "Recent",
    close: "Close File",
  },
} as const;

export function initialLang(): Lang {
  return navigator.language.startsWith("zh") ? "zh" : "en";
}

export function asLang(v: unknown): Lang | undefined {
  return v === "zh" || v === "en" ? v : undefined;
}
