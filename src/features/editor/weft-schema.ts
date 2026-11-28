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

export type AttrBlockKind = "story" | "link" | "entity" | "event";

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
  link: [
    {
      name: "acyclic",
      docDesc: {
        zh: "无环（DAG），任意时间切片禁止有向环（强制有向边）",
        en: "Acyclic (DAG); forbids directed cycles in every time slice (directed edges only)",
      },
    },
    {
      name: "asymmetric",
      docDesc: {
        zh: "非对称，禁止同时间段双向互指（强制有向边）",
        en: "Asymmetric; forbids bidirectional edges in the same slice (directed edges only)",
      },
    },
    {
      name: "intransitive",
      docDesc: {
        zh: "反传递，禁止存在跳级直连 A -> C（当 A -> B -> C 存在时）",
        en: "Intransitive; forbids shortcut A -> C when A -> B -> C exists",
      },
    },
    {
      name: "functional",
      docDesc: {
        zh: "多对一（源节点出度 ≤ 1），配合 acyclic 构成严格树",
        en: "Functional (out-degree ≤ 1); combines with acyclic for strict trees",
      },
    },
    {
      name: "inverse_functional",
      docDesc: {
        zh: "一对多（目标节点入度 ≤ 1），如神器唯一持有",
        en: "Inverse functional (in-degree ≤ 1), e.g. unique item ownership",
      },
    },
    {
      name: "symmetric",
      docDesc: {
        zh: '对称图，强制全部使用无向边或团（<-> / <- "..." ->），查重忽略方向',
        en: 'Symmetric graph; requires undirected edges or cliques (<-> / <- "..." ->)',
      },
    },
    {
      name: "transitive",
      docDesc: {
        zh: "可传递，供前端与查询层计算传递闭包",
        en: "Transitive; supports closure computation in views and queries",
      },
    },
    {
      name: "equivalence",
      docDesc: {
        zh: "等价图（自反+对称+传递），强制 <-> / 团，前端可合并商图节点",
        en: "Equivalence graph; requires <-> / cliques and enables quotient view",
      },
    },
    {
      name: "chronological",
      docDesc: {
        zh: "时序单调，强制有向边且前置节点时间 <= 后续节点时间（防止伏笔/阻塞时间倒流穿帮）",
        en: "Chronological; requires directed edges with source time <= target time",
      },
    },
    {
      name: "description",
      docDesc: {
        zh: "关系图说明，供 LSP Hover 展示",
        en: "Graph documentation shown in LSP Hover",
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
