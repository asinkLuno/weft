import type { Lang } from "@/i18n";

// ---- 词法规则（与 Rust src-tauri/src/weft/parser.rs 对齐） ----

export const IDENT_START_RE = /^[\p{L}_]/u;
export const IDENT_CONT_RE = /^[\p{L}\p{N}_·・]/u;
export const IDENT_PREFIX_RE = /[\p{L}\p{N}_·・]+/u;
export const IDENT_VALID_RE = /^[\p{L}\p{N}_·・]*$/u;
export const BARE_IDENT_RE = /^[\p{L}_][\p{L}\p{N}_·・]*$/u;

export const ATOMS = new Set(["true", "false", "null"]);

export const TOP_KEYWORDS = new Set([
  "story",
  "time",
  "entity",
  "link",
  "event",
  "narrative",
]);

export const NAMED_DECL_KEYWORDS = new Set([
  "time",
  "entity",
  "link",
  "event",
  "narrative",
]);

// ---- 块与内置字段定义（高亮、补全与格式说明页的单一事实来源） ----

export type AttrBlockKind = "story" | "entity" | "event";
export type BlockKind = AttrBlockKind | "link";

export type FieldValueKind =
  | "string"
  | "stringMap"
  | "time"
  | "entityList"
  | "calcPropList";

export type FieldSpec = {
  name: string;
  valueKind: FieldValueKind;
  required?: boolean;
  template: string;
  shortDesc: Record<Lang, string>;
  docDesc: Record<Lang, string>;
};

export const BLOCK_FIELDS: Record<AttrBlockKind, readonly FieldSpec[]> = {
  story: [
    {
      name: "title",
      valueKind: "string",
      required: true,
      template: 'title = "#{}"',
      shortDesc: { zh: "故事标题（必填）", en: "Story title (required)" },
      docDesc: { zh: "故事标题", en: "Story title" },
    },
    {
      name: "author",
      valueKind: "string",
      template: 'author = "#{}"',
      shortDesc: { zh: "作者名", en: "Author name" },
      docDesc: { zh: "作者名", en: "Author name" },
    },
    {
      name: "description",
      valueKind: "string",
      template: 'description = "#{}"',
      shortDesc: { zh: "描述文本", en: "Description text" },
      docDesc: {
        zh: "故事简介（支持三引号多行文本）",
        en: "Short synopsis (supports triple-quoted strings)",
      },
    },
    {
      name: "date_mode",
      valueKind: "string",
      template: 'date_mode = "#{}"',
      shortDesc: { zh: "历法 Rune 脚本路径", en: "Calendar Rune script path" },
      docDesc: {
        zh: "历法 Rune 脚本路径，相对本 .weft；省略或留空使用内置公历",
        en: "Calendar Rune script path relative to this .weft file; omit for built-in Gregorian",
      },
    },
    {
      name: "calculable_props",
      valueKind: "stringMap",
      template: 'calculable_props = {\n  #{id} = "#{script.rn}"\n}',
      shortDesc: { zh: "可计算属性配置", en: "Calculable props config" },
      docDesc: {
        zh: '可计算属性注册表：{ ID = ".rn 路径" }',
        en: 'Calculable prop registry: { id = ".rn path" }',
      },
    },
  ],
  entity: [
    {
      name: "description",
      valueKind: "string",
      template: 'description = "#{}"',
      shortDesc: { zh: "描述文本", en: "Description text" },
      docDesc: { zh: "实体描述", en: "Entity description" },
    },
    {
      name: "avatar",
      valueKind: "string",
      template: 'avatar = "#{}"',
      shortDesc: { zh: "头像图片路径", en: "Avatar image path" },
      docDesc: {
        zh: "头像图片路径，相对本 .weft；省略时显示 id 缩写",
        en: "Avatar image path relative to this .weft file; falls back to id initials",
      },
    },
    {
      name: "base_time",
      valueKind: "time",
      template: "base_time = [#{}]",
      shortDesc: { zh: "基准时间表达式", en: "Base time expression" },
      docDesc: {
        zh: "基准时间表达式，可计算属性的输入",
        en: "Base time expression, input of calculable props",
      },
    },
    {
      name: "belong_to",
      valueKind: "entityList",
      template: "belong_to = [#{}]",
      shortDesc: { zh: "归属上级实体列表", en: "Parent entity list" },
      docDesc: {
        zh: "上级实体 id 列表，构成归属 DAG",
        en: "Parent entity ids, forming an acyclic belong graph",
      },
    },
    {
      name: "calculable_props",
      valueKind: "calcPropList",
      template: "calculable_props = [#{}]",
      shortDesc: { zh: "可计算属性列表", en: "Calculable prop ids" },
      docDesc: {
        zh: "要计算的可计算属性 ID 列表",
        en: "Calculable prop ids to compute",
      },
    },
  ],
  event: [
    {
      name: "start",
      valueKind: "time",
      required: true,
      template: "start = [#{}]",
      shortDesc: { zh: "开始时间（必填）", en: "Start time (required)" },
      docDesc: { zh: "开始时间表达式", en: "Start time expression" },
    },
    {
      name: "end",
      valueKind: "time",
      template: "end = start + [0, 0, 0, #{1:2}]",
      shortDesc: { zh: "结束时间（可选）", en: "End time (optional)" },
      docDesc: {
        zh: "结束时间表达式，不能早于开始时间",
        en: "End time expression; must not precede start",
      },
    },
    {
      name: "entities",
      valueKind: "entityList",
      template: "entities = [#{}]",
      shortDesc: { zh: "参与实体列表", en: "Participating entities" },
      docDesc: { zh: "参与实体 id 列表", en: "Participating entity ids" },
    },
    {
      name: "description",
      valueKind: "string",
      template: 'description = "#{}"',
      shortDesc: { zh: "描述文本", en: "Description text" },
      docDesc: { zh: "事件描述", en: "Event description" },
    },
  ],
};

export const BUILTIN_PROPS = new Set(
  Object.values(BLOCK_FIELDS).flatMap((fields) => fields.map((f) => f.name)),
);

export function findFieldSpec(
  block: BlockKind,
  fieldName: string,
): FieldSpec | undefined {
  if (block === "link") return undefined;
  return BLOCK_FIELDS[block].find((f) => f.name === fieldName);
}
