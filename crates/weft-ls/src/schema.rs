//! WEFT 块与内置字段 Schema：补全与 Hover 文档的单一事实来源。

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttrBlockKind {
    Story,
    Entity,
    Event,
    Link,
    LinkEdge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockKind {
    Story,
    Entity,
    Event,
    Link,
    LinkEdge,
}

impl BlockKind {
    pub fn as_attr_block(self) -> Option<AttrBlockKind> {
        match self {
            Self::Story => Some(AttrBlockKind::Story),
            Self::Entity => Some(AttrBlockKind::Entity),
            Self::Event => Some(AttrBlockKind::Event),
            Self::Link => Some(AttrBlockKind::Link),
            Self::LinkEdge => Some(AttrBlockKind::LinkEdge),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldValueKind {
    String,
    Bool,
    StringMap,
    Time,
    EntityList,
    CalcPropList,
}

#[derive(Clone, Copy, Debug)]
pub struct FieldSpec {
    pub name: &'static str,
    pub value_kind: FieldValueKind,
    pub required: bool,
    /// LSP snippet 模板（例如 `title = "$1"`）。
    pub snippet: &'static str,
    pub short_zh: &'static str,
    pub short_en: &'static str,
    pub doc_zh: &'static str,
    pub doc_en: &'static str,
}

impl FieldSpec {
    pub fn short(&self, lang: &str) -> &'static str {
        if lang == "zh" {
            self.short_zh
        } else {
            self.short_en
        }
    }

    pub fn doc(&self, lang: &str) -> &'static str {
        if lang == "zh" {
            self.doc_zh
        } else {
            self.doc_en
        }
    }
}

pub const STORY_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        name: "title",
        value_kind: FieldValueKind::String,
        required: true,
        snippet: "title = \"$1\"",
        short_zh: "故事标题（必填）",
        short_en: "Story title (required)",
        doc_zh: "故事标题（`story` 块唯一必填字段）。",
        doc_en: "Story title (the only required field in `story`).",
    },
    FieldSpec {
        name: "author",
        value_kind: FieldValueKind::String,
        required: false,
        snippet: "author = \"$1\"",
        short_zh: "作者名",
        short_en: "Author name",
        doc_zh: "故事作者。",
        doc_en: "Author of the story.",
    },
    FieldSpec {
        name: "description",
        value_kind: FieldValueKind::String,
        required: false,
        snippet: "description = \"$1\"",
        short_zh: "故事简介",
        short_en: "Story description",
        doc_zh: "故事简介（支持 `\"\"\"...\"\"\"` 多行字符串）。",
        doc_en: "Short synopsis (supports `\"\"\"...\"\"\"` multiline strings).",
    },
    FieldSpec {
        name: "date_mode",
        value_kind: FieldValueKind::String,
        required: false,
        snippet: "date_mode = \"$1\"",
        short_zh: "历法 Rune 脚本路径",
        short_en: "Calendar Rune script path",
        doc_zh: "历法 `.rn` 脚本路径（相对当前 `.weft` 文件）；省略则使用内置公历。",
        doc_en: "Calendar `.rn` script path relative to this `.weft` file; omit for built-in Gregorian.",
    },
    FieldSpec {
        name: "calculable_props",
        value_kind: FieldValueKind::StringMap,
        required: false,
        snippet: "calculable_props = {\n  ${1:id} = \"${2:script.rn}\"\n}",
        short_zh: "可计算属性注册表",
        short_en: "Calculable prop registry",
        doc_zh: "可计算属性注册表：`{ ID = \"路径.rn\" }`，供实体通过 `calculable_props = [ID]` 启用。",
        doc_en: "Calculable property registry `{ id = \"script.rn\" }`, referenced by entities.",
    },
];

pub const LINK_GRAPH_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        name: "acyclic",
        value_kind: FieldValueKind::Bool,
        required: false,
        snippet: "acyclic = true",
        short_zh: "无环（DAG）",
        short_en: "Acyclic (DAG)",
        doc_zh: "任意时间切片内不存在有向环（强制有向边，隐含反自反、非对称），成环报 `graph-cycle`。",
        doc_en: "Forbids directed cycles in every atomic time slice (requires directed edges; forms a DAG).",
    },
    FieldSpec {
        name: "asymmetric",
        value_kind: FieldValueKind::Bool,
        required: false,
        snippet: "asymmetric = true",
        short_zh: "非对称（禁止双向互指）",
        short_en: "Asymmetric (no reverse edge)",
        doc_zh: "禁止同时间段双向互指（强制有向边，允许长度 ≥ 3 的环），违规报 `asymmetry-conflict`。",
        doc_en: "Forbids bidirectional edges (A -> B and B -> A) in the same time slice.",
    },
    FieldSpec {
        name: "intransitive",
        value_kind: FieldValueKind::Bool,
        required: false,
        snippet: "intransitive = true",
        short_zh: "反传递（禁止跳级直连）",
        short_en: "Intransitive (no shortcut edges)",
        doc_zh: "若存在 A -> B 与 B -> C，则禁止存在 A -> C，违规报 `intransitivity-violation`。",
        doc_en: "Forbids shortcut edge A -> C when A -> B and B -> C exist in the same slice.",
    },
    FieldSpec {
        name: "functional",
        value_kind: FieldValueKind::Bool,
        required: false,
        snippet: "functional = true",
        short_zh: "多对一（源节点出度 ≤ 1）",
        short_en: "Functional (out-degree <= 1)",
        doc_zh: "任意时间切片同一源节点出度不超过 1（配合 `acyclic` 构成严格树）。",
        doc_en: "Limits each source node to at most one outgoing target per time slice.",
    },
    FieldSpec {
        name: "inverse_functional",
        value_kind: FieldValueKind::Bool,
        required: false,
        snippet: "inverse_functional = true",
        short_zh: "一对多（目标节点入度 ≤ 1）",
        short_en: "Inverse functional (in-degree <= 1)",
        doc_zh: "任意时间切片同一目标节点入度不超过 1（如物品唯一持有）。",
        doc_en: "Limits each target node to at most one incoming source per time slice.",
    },
    FieldSpec {
        name: "symmetric",
        value_kind: FieldValueKind::Bool,
        required: false,
        snippet: "symmetric = true",
        short_zh: "对称图（强制 <-> 或团）",
        short_en: "Symmetric (requires <-> or clique)",
        doc_zh: "整张图强制为无向对称图（仅允许 `<->`、`<- \"...\" ->` 或团），查重忽略方向。",
        doc_en: "Requires undirected edges (`<->`, `<- \"...\" ->`, or cliques) across this graph.",
    },
    FieldSpec {
        name: "transitive",
        value_kind: FieldValueKind::Bool,
        required: false,
        snippet: "transitive = true",
        short_zh: "可传递（支持传递闭包）",
        short_en: "Transitive (supports closure)",
        doc_zh: "标记可传递，供前端与查询层计算传递闭包。",
        doc_en: "Marks graph as transitive for closure computation in views and queries.",
    },
    FieldSpec {
        name: "equivalence",
        value_kind: FieldValueKind::Bool,
        required: false,
        snippet: "equivalence = true",
        short_zh: "等价图（商图坍缩）",
        short_en: "Equivalence (quotient graph)",
        doc_zh: "等价关系图（自反 + 对称 + 传递），强制使用 `<->` / 团，前端可合并为商图节点。",
        doc_en: "Equivalence graph requiring `<->` / cliques; enables quotient-node merging in UI.",
    },
    FieldSpec {
        name: "chronological",
        value_kind: FieldValueKind::Bool,
        required: false,
        snippet: "chronological = true",
        short_zh: "时序单调（防时间穿帮）",
        short_en: "Chronological (time-ordered)",
        doc_zh: "强制有向边，校验前置节点时间不晚于后续节点时间（如伏笔与阻塞关系防止时间倒流穿帮），违规报 `chronological-violation`。",
        doc_en: "Requires directed edges where source time <= target time (prevents chronological plot holes in foreshadowing/blocking graphs).",
    },
    FieldSpec {
        name: "description",
        value_kind: FieldValueKind::String,
        required: false,
        snippet: "description = \"$1\"",
        short_zh: "关系图说明",
        short_en: "Graph description",
        doc_zh: "该张关系图的文档说明，供 LSP Hover 展示。",
        doc_en: "Documentation note for this link graph, displayed in LSP Hover.",
    },
];

pub const ENTITY_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        name: "description",
        value_kind: FieldValueKind::String,
        required: false,
        snippet: "description = \"$1\"",
        short_zh: "实体描述",
        short_en: "Entity description",
        doc_zh: "实体的文字描述（支持多行字符串）。",
        doc_en: "Description of the entity (supports multiline strings).",
    },
    FieldSpec {
        name: "avatar",
        value_kind: FieldValueKind::String,
        required: false,
        snippet: "avatar = \"$1\"",
        short_zh: "头像图片路径",
        short_en: "Avatar image path",
        doc_zh: "头像图片路径（相对 `.weft` 文件），支持 bmp/gif/ico/jpeg/jpg/png/svg/webp。",
        doc_en: "Avatar image path relative to this `.weft` file (bmp/gif/ico/jpeg/jpg/png/svg/webp).",
    },
    FieldSpec {
        name: "base_time",
        value_kind: FieldValueKind::Time,
        required: false,
        snippet: "base_time = [$1]",
        short_zh: "基准时间表达式",
        short_en: "Base time expression",
        doc_zh: "实体的出生/起始基准时间表达式，可作为时间引用或可计算属性的输入。",
        doc_en: "Base/birth time expression for this entity; usable in time refs and calculable props.",
    },
    FieldSpec {
        name: "calculable_props",
        value_kind: FieldValueKind::CalcPropList,
        required: false,
        snippet: "calculable_props = [$1]",
        short_zh: "启用的可计算属性",
        short_en: "Calculable prop ids",
        doc_zh: "为该实体启用的可计算属性 ID 列表（须先在 `story.calculable_props` 注册）。",
        doc_en: "Calculable property IDs to evaluate (must be registered in `story.calculable_props`).",
    },
];

pub const EVENT_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        name: "start",
        value_kind: FieldValueKind::Time,
        required: true,
        snippet: "start = [$1]",
        short_zh: "开始时间（必填）",
        short_en: "Start time (required)",
        doc_zh: "事件开始时间表达式（必填）。",
        doc_en: "Event start time expression (required).",
    },
    FieldSpec {
        name: "end",
        value_kind: FieldValueKind::Time,
        required: false,
        snippet: "end = start + [0, 0, 0, ${1:2}]",
        short_zh: "结束时间（可选）",
        short_en: "End time (optional)",
        doc_zh: "事件结束时间表达式（可选），不能早于 `start`；块内可直接写 `start` 引用自身开始时间。",
        doc_en: "Optional end time expression; must not precede `start`. May reference self `start`.",
    },
    FieldSpec {
        name: "entities",
        value_kind: FieldValueKind::EntityList,
        required: false,
        snippet: "entities = [$1]",
        short_zh: "参与实体列表",
        short_en: "Participating entities",
        doc_zh: "参与本事件的已声明实体列表。",
        doc_en: "List of declared entities participating in this event.",
    },
    FieldSpec {
        name: "description",
        value_kind: FieldValueKind::String,
        required: false,
        snippet: "description = \"$1\"",
        short_zh: "事件描述",
        short_en: "Event description",
        doc_zh: "事件的文字描述（支持多行字符串）。",
        doc_en: "Description of the event (supports multiline strings).",
    },
];

pub const LINK_EDGE_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        name: "start",
        value_kind: FieldValueKind::Time,
        required: false,
        snippet: "start = $1",
        short_zh: "关系生效时间（缺省 -∞）",
        short_en: "Relation start time (default -∞)",
        doc_zh: "关系生效时间表达式；省略表示从负无穷生效，支持所有 WEFT 时间表达式与块内 `end` 自引用。",
        doc_en: "When this relation edge begins (defaults to -∞); supports all time expressions.",
    },
    FieldSpec {
        name: "end",
        value_kind: FieldValueKind::Time,
        required: false,
        snippet: "end = $1",
        short_zh: "关系终止时间（缺省 +∞）",
        short_en: "Relation end time (default +∞)",
        doc_zh: "关系终止时间表达式；省略表示持续至正无穷，不能早于 `start`。",
        doc_en: "When this relation edge ends (defaults to +∞); must not precede `start`.",
    },
    FieldSpec {
        name: "description",
        value_kind: FieldValueKind::String,
        required: false,
        snippet: "description = \"$1\"",
        short_zh: "关系边描述",
        short_en: "Link edge description",
        doc_zh: "该条关系边的文字说明（支持多行字符串）。",
        doc_en: "Description for this specific relation edge.",
    },
];

pub fn fields_for(block: AttrBlockKind) -> &'static [FieldSpec] {
    match block {
        AttrBlockKind::Story => STORY_FIELDS,
        AttrBlockKind::Entity => ENTITY_FIELDS,
        AttrBlockKind::Event => EVENT_FIELDS,
        AttrBlockKind::Link => LINK_GRAPH_FIELDS,
        AttrBlockKind::LinkEdge => LINK_EDGE_FIELDS,
    }
}

pub fn find_field(block: BlockKind, name: &str) -> Option<&'static FieldSpec> {
    let attr_block = block.as_attr_block()?;
    fields_for(attr_block).iter().find(|f| f.name == name)
}

pub fn keyword_doc(word: &str, lang: &str) -> Option<&'static str> {
    let zh = lang == "zh";
    match word {
        "story" => Some(if zh {
            "`story { ... }` — 故事元信息块（每个文档必填且唯一）。"
        } else {
            "`story { ... }` — Story metadata block (required and unique per document)."
        }),
        "time" => Some(if zh {
            "`time <名称> = <时间表达式>` — 独立命名时间锚点，可在任意时间字段中复用。"
        } else {
            "`time <name> = <expr>` — Named time anchor reusable in any time expression."
        }),
        "entity" => Some(if zh {
            "`entity <名称> { ... }` — 实体声明（人物、组织、地点、道具等）；内置字段外的键透传到 `extra_props`。"
        } else {
            "`entity <name> { ... }` — Entity declaration (character, group, place, item, etc.); extra keys pass through as `extra_props`."
        }),
        "link" => Some(if zh {
            "`link <图名> { ... }` — 同质关系图：纯 `entity` 时变状态图或纯 `event` 剧情图（支持有向边 `->` / `-- \"标签\" ->`、无向边 `<->` / `<- \"标签\" ->`、团 `<-> [...]` 与批量端点 `[...] -> ...`）。"
        } else {
            "`link <graph> { ... }` — Homogeneous relation graph over either `entity` nodes (temporal) or `event` nodes (plot DAG); supports directed, symmetric, cliques, and batch endpoints."
        }),
        "event" => Some(if zh {
            "`event <名称> { ... }` — 事件声明，必填 `start`，与 `entity` 共享统一节点命名空间。"
        } else {
            "`event <name> { ... }` — Event declaration (`start` required); shares node namespace with `entity`."
        }),
        "events" => Some(if zh {
            "`events <名称> = [事件1, 事件2]` — 有序事件列表，列表顺序即时间线展示顺序。"
        } else {
            "`events <name> = [event1, event2]` — Ordered event list defining timeline display order."
        }),
        _ => None,
    }
}
