import type { Lang } from "@/i18n";

// ---- 语法高亮与格式说明页共用的轻量语言定义 ----

export const IDENT_START_RE = /^[\p{L}_]/u;
export const IDENT_CONT_RE = /^[\p{L}\p{N}_·・]/u;

export const ATOMS = new Set(["true", "false", "null"]);

export const TOP_KEYWORDS = new Set([
  "story",
  "time",
  "entity",
  "link",
  "event",
  "events",
]);

export const NAMED_DECL_KEYWORDS = new Set([
  "time",
  "entity",
  "link",
  "event",
  "events",
]);

export type AttrBlockKind = "story" | "entity" | "event";

export type FieldDocSpec = {
  name: string;
  required?: boolean;
  docDesc: Record<Lang, string>;
};

export const BLOCK_FIELDS: Record<AttrBlockKind, readonly FieldDocSpec[]> = {
  story: [
    {
      name: "title",
      required: true,
      docDesc: { zh: "故事标题", en: "Story title" },
    },
    {
      name: "author",
      docDesc: { zh: "作者名", en: "Author name" },
    },
    {
      name: "description",
      docDesc: {
        zh: "故事简介（支持三引号多行文本）",
        en: "Short synopsis (supports triple-quoted strings)",
      },
    },
    {
      name: "date_mode",
      docDesc: {
        zh: "历法 Rune 脚本路径，相对本 .weft；省略或留空使用内置公历",
        en: "Calendar Rune script path relative to this .weft file; omit for built-in Gregorian",
      },
    },
    {
      name: "calculable_props",
      docDesc: {
        zh: '可计算属性注册表：{ ID = ".rn 路径" }',
        en: 'Calculable prop registry: { id = ".rn path" }',
      },
    },
  ],
  entity: [
    {
      name: "description",
      docDesc: { zh: "实体描述", en: "Entity description" },
    },
    {
      name: "avatar",
      docDesc: {
        zh: "头像图片路径，相对本 .weft；省略时显示 id 缩写",
        en: "Avatar image path relative to this .weft file; falls back to id initials",
      },
    },
    {
      name: "base_time",
      docDesc: {
        zh: "基准时间表达式，可计算属性的输入",
        en: "Base time expression, input of calculable props",
      },
    },
    {
      name: "belong_to",
      docDesc: {
        zh: "上级实体 id 列表，构成归属 DAG",
        en: "Parent entity ids, forming an acyclic belong graph",
      },
    },
    {
      name: "calculable_props",
      docDesc: {
        zh: "要计算的可计算属性 ID 列表",
        en: "Calculable prop ids to compute",
      },
    },
  ],
  event: [
    {
      name: "start",
      required: true,
      docDesc: { zh: "开始时间表达式", en: "Start time expression" },
    },
    {
      name: "end",
      docDesc: {
        zh: "结束时间表达式，不能早于开始时间",
        en: "End time expression; must not precede start",
      },
    },
    {
      name: "entities",
      docDesc: { zh: "参与实体 id 列表", en: "Participating entity ids" },
    },
    {
      name: "description",
      docDesc: { zh: "事件描述", en: "Event description" },
    },
  ],
};

export const BUILTIN_PROPS = new Set(
  Object.values(BLOCK_FIELDS).flatMap((fields) => fields.map((f) => f.name)),
);
