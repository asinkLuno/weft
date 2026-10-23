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
    unknown_link_entity: "entity_link `{group}` 引用了不存在的实体 `{entity}`",
    unknown_event_entity: "event `{event}` 引用了不存在的实体 `{entity}`",
    unknown_narrative_event:
      "narrative `{narrative}` 引用了不存在的事件 `{event}`",
    entity_unknown_parent:
      "entity `{entity}` belong_to 引用了不存在的实体 `{target}`",
    entity_cycle: "entity `{entity}` 的 belong_to 形成了循环",
    invalid_avatar_path:
      "entity `{entity}` 的 avatar `{path}` 必须指向已存在的图片文件",
    time_nonempty_list: "时间必须是非空列表",
    time_integer_component: "时间分量必须是整数",
    event_end_before_start: "事件 `{event}` 的结束时间早于开始时间",
    invalid_calendar_path: "历法路径 `{path}` 必须指向现有的 .rhai 文件",
    invalid_calculable_prop_path:
      "可计算属性 `{prop}` 的路径 `{path}` 必须指向 .rhai 文件",
    calculable_prop_file_read: "读取可计算属性 `{prop}` 的脚本失败：{source}",
    calculable_prop_compile: "可计算属性 `{prop}` 编译失败：{source}",
    calculable_prop_missing_interface:
      "可计算属性 `{prop}` 缺少公开接口：{functions}",
    unknown_calculable_prop: "引用了未注册的可计算属性 `{prop}`",
    calculable_prop_argument: "可计算属性 `{prop}` 入参转换失败：{source}",
    calculable_prop_call: "可计算属性 `{prop}` 的 `{entry}` 执行失败：{source}",
    calculable_prop_invalid_return:
      "可计算属性 `{prop}` 的 `{entry}` 返回值无效，预期 {expected}",
    calendar_components_out_of_range:
      "历法 {calendar} 必须声明整数常量 COMPONENTS，取值为 1 到 64",
    calendar_file_read: "读取历法 `{calendar}` 的脚本失败：{source}",
    calendar_compile: "历法 `{calendar}` 编译失败：{source}",
    calendar_call: "历法 `{calendar}` 的 `{entry}` 执行失败：{source}",
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
    unknown_link_entity:
      "entity_link `{group}` refers to unknown entity `{entity}`",
    unknown_event_entity: "event `{event}` refers to unknown entity `{entity}`",
    unknown_narrative_event:
      "narrative `{narrative}` refers to unknown event `{event}`",
    entity_unknown_parent:
      "entity `{entity}` belongs to unknown entity `{target}`",
    entity_cycle: "belong_to for entity `{entity}` forms a cycle",
    invalid_avatar_path:
      "Avatar `{path}` for entity `{entity}` must point to an existing image file",
    time_nonempty_list: "Time must be a nonempty list",
    time_integer_component: "Time components must be integers",
    event_end_before_start: "End time is before start time for event `{event}`",
    invalid_calendar_path:
      "Calendar path `{path}` must point to an existing .rhai file",
    invalid_calculable_prop_path:
      "Path `{path}` for calculable prop `{prop}` must point to a .rhai file",
    calculable_prop_file_read:
      "Failed to read calculable prop `{prop}`: {source}",
    calculable_prop_compile:
      "Failed to compile calculable prop `{prop}`: {source}",
    calculable_prop_missing_interface:
      "Calculable prop `{prop}` is missing public interfaces: {functions}",
    unknown_calculable_prop: "Unknown calculable prop `{prop}`",
    calculable_prop_argument:
      "Failed to convert arguments for calculable prop `{prop}`: {source}",
    calculable_prop_call:
      "Error running `{entry}` of calculable prop `{prop}`: {source}",
    calculable_prop_invalid_return:
      "`{entry}` of calculable prop `{prop}` returned an invalid value; expected {expected}",
    calendar_components_out_of_range:
      "Calendar {calendar} must declare an integer constant COMPONENTS between 1 and 64",
    calendar_file_read:
      "Failed to read the script of calendar `{calendar}`: {source}",
    calendar_compile: "Failed to compile calendar `{calendar}`: {source}",
    calendar_call: "Error running `{entry}` of calendar `{calendar}`: {source}",
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
    workspace: "故事工作台",
    welcomeTitle: "让故事的脉络\n清晰可见。",
    welcomeDescription:
      "打开故事文件，梳理人物与实体的关系，沿着时间线阅读每一条叙事。",
    recentDescription: "继续阅读你的故事。",
    recentEmpty: "从第一个故事开始。打开的文件会留在这里。",
    fileHint: "支持 .yaml 和 .yml 文件，保存修改后视图自动更新。",
    fileWatching: "正在跟随文件更新",
    fileError: "文件需要检查",
    entityEmpty: "暂无实体，请在故事文件中添加实体。",
    loading: "正在载入实体…",
    tab1: "故事",
    tab2: "实体",
    tab4: "叙事",
    author: "作者",
    calendar: "历法",
    belongTo: "归属",
    linkGroup: "关系",
    baseTime: "基准时间",
    narrativeEmpty: "暂无叙事",
    ganttEvent: "事件",
    open: "打开 YAML 文件",
    recent: "最近打开",
    close: "关闭文件",
  },
  en: {
    workspace: "Story workspace",
    welcomeTitle: "Find the threads\nof your story.",
    welcomeDescription:
      "Open a story file to explore its people, trace their connections, and follow each narrative through time.",
    recentDescription: "Pick up where you left off.",
    recentEmpty: "Start with your first story. Opened files will appear here.",
    fileHint:
      "Supports .yaml and .yml files. Views update when you save changes.",
    fileWatching: "Following file changes",
    fileError: "File needs attention",
    entityEmpty: "No entities yet. Add entities to your story file.",
    loading: "Loading entities…",
    tab1: "Story",
    tab2: "Entities",
    tab4: "Narratives",
    author: "Author",
    calendar: "Calendar",
    belongTo: "Belongs to",
    linkGroup: "Links",
    baseTime: "Base time",
    narrativeEmpty: "No narratives",
    ganttEvent: "Event",
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
