import type {
  CompileDiagnostic,
  ValidationError,
} from "./generated/validation-error";

export type { CompileDiagnostic };
export type Lang = "zh" | "en";

type ValidationCode = ValidationError["code"];

const validationMessages = {
  zh: {
    syntax_error: "语法错误：{source}",
    missing_story: "缺少必填的 story 块",
    duplicate_story: "重复声明了 story 块",
    duplicate_entity: "重复声明了实体 `{entity}`",
    duplicate_event: "重复声明了事件 `{event}`",
    duplicate_narrative: "重复声明了叙事线 `{narrative}`",
    duplicate_time_anchor: "重复声明了时间锚点 `{name}`",
    missing_field: "{section} 缺少必填字段 `{field}`",
    invalid_field: "{section} 的字段 `{field}` 值无效：{reason}",
    unknown_field: "{section} 包含未知字段 `{field}`",
    unknown_time_ref: "引用了不存在的时间 `{reference}`",
    time_cycle: "时间引用 `{target}` 形成了循环",
    unknown_link_entity: "link `{group}` 引用了不存在的实体 `{entity}`",
    unknown_event_entity: "event `{event}` 引用了不存在的实体 `{entity}`",
    unknown_narrative_event:
      "narrative `{narrative}` 引用了不存在的事件 `{event}`",
    entity_unknown_parent:
      "entity `{entity}` belong_to 引用了不存在的实体 `{target}`",
    entity_cycle: "entity `{entity}` 的 belong_to 形成了循环",
    invalid_avatar_path:
      "entity `{entity}` 的 avatar `{path}` 必须指向已存在的图片文件",
    time_nonempty_list: "时间必须是非空整数列表或有效时间表达式",
    time_integer_component: "时间分量必须是整数",
    time_nesting_too_deep: "时间锚点嵌套超过最大深度 {maximum}",
    event_end_before_start: "事件 `{event}` 的结束时间早于开始时间",
    invalid_calendar_path: "历法路径 `{path}` 必须指向现有的 .rn 文件",
    invalid_calculable_prop_path:
      "可计算属性 `{prop}` 的路径 `{path}` 必须指向现有的 .rn 文件",
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
      "历法 `{calendar}` 的分量数必须在 1 到 64 之间",
    calendar_file_read: "读取历法 `{calendar}` 的脚本失败：{source}",
    calendar_compile: "历法 `{calendar}` 编译失败：{source}",
    calendar_missing_interface: "历法 `{calendar}` 缺少公开接口：{functions}",
    calendar_call: "历法 `{calendar}` 的 `{entry}` 执行失败：{source}",
    calendar_invalid_return: "历法 `{calendar}` 的 `{entry}` 返回值无效",
    too_many_time_components:
      "时间分量过多：{actual} 个，该历法最多 {maximum} 个",
    normalize_wrong_component_count: "from_tick 必须返回 {expected} 个时间分量",
    time_offset_out_of_range: "时间偏移超出整数范围",
    time_component_below_minimum:
      "第 {position} 个时间分量为 {value}，低于该历法允许的最小值 {minimum}",
    calendar_round_trip_mismatch:
      "历法 `{calendar}` 的 to_tick(from_tick(tick)) 未能还原同一刻度",
  },
  en: {
    syntax_error: "Syntax error: {source}",
    missing_story: "Missing required story block",
    duplicate_story: "Duplicate story block",
    duplicate_entity: "Duplicate entity `{entity}`",
    duplicate_event: "Duplicate event `{event}`",
    duplicate_narrative: "Duplicate narrative `{narrative}`",
    duplicate_time_anchor: "Duplicate time anchor `{name}`",
    missing_field: "Missing required field `{field}` in {section}",
    invalid_field: "Invalid value for `{field}` in {section}: {reason}",
    unknown_field: "Unknown field `{field}` in {section}",
    unknown_time_ref: "Unknown time reference `{reference}`",
    time_cycle: "Time reference `{target}` forms a cycle",
    unknown_link_entity: "link `{group}` refers to unknown entity `{entity}`",
    unknown_event_entity: "event `{event}` refers to unknown entity `{entity}`",
    unknown_narrative_event:
      "narrative `{narrative}` refers to unknown event `{event}`",
    entity_unknown_parent:
      "entity `{entity}` belongs to unknown entity `{target}`",
    entity_cycle: "belong_to for entity `{entity}` forms a cycle",
    invalid_avatar_path:
      "Avatar `{path}` for entity `{entity}` must point to an existing image file",
    time_nonempty_list:
      "Time must be a nonempty integer list or valid time expression",
    time_integer_component: "Time components must be integers",
    time_nesting_too_deep:
      "Time anchor nesting exceeds maximum depth {maximum}",
    event_end_before_start: "End time is before start time for event `{event}`",
    invalid_calendar_path:
      "Calendar path `{path}` must point to an existing .rn file",
    invalid_calculable_prop_path:
      "Path `{path}` for calculable prop `{prop}` must point to an existing .rn file",
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
      "Calendar `{calendar}` component count must be between 1 and 64",
    calendar_file_read:
      "Failed to read the script of calendar `{calendar}`: {source}",
    calendar_compile: "Failed to compile calendar `{calendar}`: {source}",
    calendar_missing_interface:
      "Calendar `{calendar}` is missing public interfaces: {functions}",
    calendar_call: "Error running `{entry}` of calendar `{calendar}`: {source}",
    calendar_invalid_return:
      "`{entry}` of calendar `{calendar}` returned an invalid value",
    too_many_time_components:
      "Too many time components: {actual}; this calendar allows at most {maximum}",
    normalize_wrong_component_count:
      "from_tick must return {expected} time components",
    time_offset_out_of_range: "Time offset is outside the integer range",
    time_component_below_minimum:
      "Time component {position} is {value}, below the calendar minimum {minimum}",
    calendar_round_trip_mismatch:
      "Calendar `{calendar}`: to_tick(from_tick(tick)) did not return the same tick",
  },
} satisfies Record<Lang, Record<ValidationCode, string>>;

export function formatValidationError(
  error: ValidationError,
  lang: Lang,
): string {
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

// 文案多到难维护时再换 i18next，字典结构可以原样搬过去
export const strings = {
  zh: {
    recentDescription: "继续阅读你的故事。",
    recentEmpty: "从第一个故事开始。打开的文件会留在这里。",
    fileHint: "支持 .weft 文件，保存修改后视图自动更新。",
    saved: "已保存",
    statusConflict: "磁盘文件已冲突",
    statusDeleted: "磁盘文件已删除",
    entityEmpty: "暂无实体，请在故事文件中添加实体。",
    storyEmpty: "暂无故事信息，请在编辑器中检查并修复错误。",
    loading: "正在载入实体…",
    panelEditor: "Weft",
    tab1: "故事",
    tab2: "实体",
    tab4: "叙事",
    tabInfo: "格式说明",
    author: "作者",
    calendar: "历法",
    belongTo: "归属",
    linkGroup: "关系",
    baseTime: "基准时间",
    narrativeEmpty: "暂无叙事",
    ganttEvent: "事件",
    open: "打开 Weft 文件",
    recent: "最近打开",
    close: "关闭文件",
    save: "保存",
    saving: "保存中…",
    unsaved: "未保存",
    revert: "还原",
    externalChanged: "磁盘文件已被外部修改",
    externalDeleted: "磁盘上的文件已被删除或移动",
    reloadFromDisk: "重新加载",
    overwriteDisk: "覆盖磁盘",
    recreateFile: "重新保存",
    addTab: "新增标签页",
    splitRight: "向右分屏",
    splitDown: "向下分屏",
    resetLayout: "重置布局",
    workspaceEmpty: "当前没有打开的面板",
  },
  en: {
    recentDescription: "Pick up where you left off.",
    recentEmpty: "Start with your first story. Opened files will appear here.",
    fileHint: "Supports .weft files. Views update when you save changes.",
    saved: "Saved",
    statusConflict: "Disk conflict",
    statusDeleted: "Deleted on disk",
    entityEmpty: "No entities yet. Add entities to your story file.",
    storyEmpty: "No story data yet. Check the editor for errors.",
    loading: "Loading entities…",
    panelEditor: "Weft",
    tab1: "Story",
    tab2: "Entities",
    tab4: "Narratives",
    tabInfo: "Format",
    author: "Author",
    calendar: "Calendar",
    belongTo: "Belongs to",
    linkGroup: "Links",
    baseTime: "Base time",
    narrativeEmpty: "No narratives",
    ganttEvent: "Event",
    open: "Open Weft File",
    recent: "Recent",
    close: "Close File",
    save: "Save",
    saving: "Saving…",
    unsaved: "Unsaved",
    revert: "Revert",
    externalChanged: "File changed on disk",
    externalDeleted: "File was deleted or moved on disk",
    reloadFromDisk: "Reload",
    overwriteDisk: "Overwrite",
    recreateFile: "Save to recreate",
    addTab: "New tab",
    splitRight: "Split right",
    splitDown: "Split down",
    resetLayout: "Reset layout",
    workspaceEmpty: "No panels open",
  },
} as const;

export function initialLang(): Lang {
  return navigator.language.startsWith("zh") ? "zh" : "en";
}

export function asLang(v: unknown): Lang | undefined {
  return v === "zh" || v === "en" ? v : undefined;
}
