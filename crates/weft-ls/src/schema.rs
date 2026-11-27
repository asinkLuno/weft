//! Weft 块与内置字段 Schema：补全与 Hover 文档的单一事实来源。

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttrBlockKind {
    Story,
    Entity,
    Event,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockKind {
    Story,
    Entity,
    Event,
    Link,
}

impl BlockKind {
    pub fn as_attr_block(self) -> Option<AttrBlockKind> {
        match self {
            Self::Story => Some(AttrBlockKind::Story),
            Self::Entity => Some(AttrBlockKind::Entity),
            Self::Event => Some(AttrBlockKind::Event),
            Self::Link => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldValueKind {
    String,
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
        name: "belong_to",
        value_kind: FieldValueKind::EntityList,
        required: false,
        snippet: "belong_to = [$1]",
        short_zh: "归属上级实体列表",
        short_en: "Parent entity list",
        doc_zh: "上级实体列表，构成归属有向无环图（DAG），不允许成环。",
        doc_en: "Parent entity list forming an acyclic ownership DAG.",
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

pub fn fields_for(block: AttrBlockKind) -> &'static [FieldSpec] {
    match block {
        AttrBlockKind::Story => STORY_FIELDS,
        AttrBlockKind::Entity => ENTITY_FIELDS,
        AttrBlockKind::Event => EVENT_FIELDS,
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
            "`entity <名称> { ... }` — 实体声明（人物、团体、地点、道具等）；内置字段外的键会透传到 `extra_props`。"
        } else {
            "`entity <name> { ... }` — Entity declaration (character, group, place, item); extra keys pass through as `extra_props`."
        }),
        "link" => Some(if zh {
            "`link <组名> { A -- \"关系\" -- B }` — 按组声明实体之间的有向关系边。"
        } else {
            "`link <group> { A -- \"relation\" -- B }` — Grouped pairwise entity relation edges."
        }),
        "event" => Some(if zh {
            "`event <名称> { ... }` — 事件声明，必填 `start`，支持跨事件/实体相对时间偏移。"
        } else {
            "`event <name> { ... }` — Event declaration (`start` required); supports relative time offsets."
        }),
        "events" => Some(if zh {
            "`events <名称> = [事件1, 事件2]` — 有序事件列表，列表顺序即时间线展示顺序。"
        } else {
            "`events <name> = [event1, event2]` — Ordered event list defining timeline display order."
        }),
        _ => None,
    }
}
