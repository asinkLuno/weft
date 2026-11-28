//! 统一源码区间、编译校验错误与多诊断模型。

use std::{fmt, ops::Range};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// UTF-8 字节区间（语言内核唯一位置真相；终端渲染和编辑器按需换算行列或 UTF-16）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ByteSpan {
    pub start: usize,
    pub end: usize,
}

impl ByteSpan {
    pub fn new(start: usize, end: usize) -> Self {
        Self {
            start,
            end: end.max(start),
        }
    }

    pub fn range(self) -> Range<usize> {
        self.start..self.end
    }

    /// 换算为 1-based `(line, column)`（按 Unicode 字符数计数）。
    pub fn line_col(self, src: &str) -> (usize, usize) {
        let start = clamp_char_boundary(src, self.start);
        let prefix = &src[..start];
        let line = prefix.bytes().filter(|&b| b == b'\n').count() + 1;
        let column = match prefix.rfind('\n') {
            Some(pos) => prefix[pos + 1..].chars().count() + 1,
            None => prefix.chars().count() + 1,
        };
        (line, column)
    }

    /// 换算为交付给前端 CodeMirror 的 UTF-16 `SourceSpan`。
    pub fn to_source_span(self, src: &str) -> SourceSpan {
        let start_byte = clamp_char_boundary(src, self.start);
        let end_byte = clamp_char_boundary(src, self.end.max(start_byte));
        let prefix = &src[..start_byte];
        let start = prefix.encode_utf16().count();
        let end = start + src[start_byte..end_byte].encode_utf16().count();
        let (line, column) = self.line_col(src);
        SourceSpan {
            start,
            end,
            line,
            column,
        }
    }
}

pub(crate) fn clamp_char_boundary(src: &str, offset: usize) -> usize {
    let mut safe = offset.min(src.len());
    while safe > 0 && !src.is_char_boundary(safe) {
        safe -= 1;
    }
    safe
}

/// 源码定位信息（UTF-16 偏移对齐 CodeMirror；1-based 行列号用于日志展示）。
#[derive(Clone, Debug, Serialize)]
pub struct SourceSpan {
    pub start: usize,
    pub end: usize,
    #[serde(skip)]
    pub line: usize,
    #[serde(skip)]
    pub column: usize,
}

/// 交付给前端编辑器的单条编译诊断。
#[derive(Clone, Debug, Serialize)]
pub struct CompileDiagnostic {
    pub error: ValidationError,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<SourceSpan>,
}

/// 诊断严重级别。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
}

/// 诊断的辅助定位标签（例如重复定义时指向首次定义位置）。
#[derive(Clone, Debug, Serialize)]
pub struct RelatedSpan {
    pub span: ByteSpan,
    pub message: String,
}

/// 统一的语言诊断记录（覆盖语法错误、语义校验错误与 Lint 告警）。
#[derive(Clone, Debug, Serialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub rule: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<ByteSpan>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub related: Vec<RelatedSpan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    #[serde(skip)]
    pub error: Option<ValidationError>,
}

impl Diagnostic {
    pub fn from_validation(error: ValidationError, span: Option<ByteSpan>) -> Self {
        let rule = error.rule();
        let message = error.to_string();
        Self {
            severity: Severity::Error,
            rule,
            message,
            span,
            related: Vec::new(),
            help: None,
            suggestion: None,
            error: Some(error),
        }
    }

    pub fn warning(rule: &'static str, message: impl Into<String>, span: Option<ByteSpan>) -> Self {
        Self {
            severity: Severity::Warning,
            rule,
            message: message.into(),
            span,
            related: Vec::new(),
            help: None,
            suggestion: None,
            error: None,
        }
    }

    pub fn with_related(mut self, span: ByteSpan, message: impl Into<String>) -> Self {
        self.related.push(RelatedSpan {
            span,
            message: message.into(),
        });
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        let suggestion = suggestion.into();
        self.help = Some(format!("did you mean `{suggestion}`?"));
        self.suggestion = Some(suggestion);
        self
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }

    pub fn localize(&mut self, lang: &str) {
        if let Some(err) = &self.error {
            self.message = err.message_for(lang);
        }
        if let Some(suggestion) = &self.suggestion {
            self.help = Some(if lang == "zh" {
                format!("是不是想写 `{suggestion}`？")
            } else {
                format!("did you mean `{suggestion}`?")
            });
        }
    }

    pub fn to_compile_diagnostic(&self, src: &str) -> Option<CompileDiagnostic> {
        let error = self.error.clone()?;
        let span = self.span.map(|s| s.to_source_span(src));
        Some(CompileDiagnostic { error, span })
    }
}

// 校验错误的唯一代码定义。
#[derive(Clone, Debug, Error, Serialize)]
#[serde(tag = "code", content = "params", rename_all = "snake_case")]
pub enum ValidationError {
    #[error("syntax error: {details}")]
    SyntaxError {
        #[serde(rename = "source")]
        details: String,
    },
    #[error("missing required story block")]
    MissingStory,
    #[error("duplicate story block")]
    DuplicateStory,
    #[error("duplicate entity `{entity}`")]
    DuplicateEntity { entity: String },
    #[error("duplicate event `{event}`")]
    DuplicateEvent { event: String },
    #[error("duplicate link graph `{graph}`")]
    DuplicateLinkGraph { graph: String },
    #[error("symbol `{name}` is already declared as {existing_kind} and cannot also be declared as {new_kind}")]
    DuplicateSymbol {
        name: String,
        existing_kind: String,
        new_kind: String,
    },
    #[error("duplicate events `{events}`")]
    DuplicateEvents { events: String },
    #[error("duplicate time anchor `{name}`")]
    DuplicateTimeAnchor { name: String },
    #[error("link graph `{graph}` enables mutually exclusive properties `{attr_a}` and `{attr_b}`")]
    InvalidGraphConfig {
        graph: String,
        attr_a: String,
        attr_b: String,
    },
    #[error("missing required field `{field}` in {section}")]
    MissingField { section: String, field: String },
    #[error("invalid value for field `{field}` in {section}: {reason}")]
    InvalidField {
        section: String,
        field: String,
        reason: String,
    },
    #[error("unknown field `{field}` in {section}")]
    UnknownField { section: String, field: String },
    #[error("unknown time reference `{reference}`")]
    UnknownTimeRef { reference: String },
    #[error("time reference `{target}` forms a cycle")]
    TimeCycle { target: String },
    #[error("link `{graph}` refers to unknown node `{entity}`")]
    UnknownLinkEntity { graph: String, entity: String },
    #[error("link `{graph}` mixes {expected_kind} and {found_kind} nodes (`{node}`); a link graph must connect only entities or only events")]
    MixedLinkNodeKinds {
        graph: String,
        expected_kind: String,
        found_kind: String,
        node: String,
    },
    #[error("event link graph `{graph}` cannot enable `equivalence`")]
    EventGraphEquivalence { graph: String },
    #[error("edges in event link graph `{graph}` cannot specify `start` or `end`")]
    EventLinkHasTimeBounds { graph: String },
    #[error("edges in equivalence link graph `{graph}` cannot specify `start` or `end`")]
    EquivalenceLinkHasTimeBounds { graph: String },
    #[error("chronological link `{graph}` violated: `{from}` ({from_time}) occurs after `{to}` ({to_time})")]
    ChronologicalViolation {
        graph: String,
        from: String,
        from_time: String,
        to: String,
        to_time: String,
    },
    #[error("clique in link `{graph}` requires at least 2 nodes, got {count}")]
    InvalidClique { graph: String, count: usize },
    #[error("endpoint list in link `{graph}` cannot be empty")]
    EmptyLinkEndpoint { graph: String },
    #[error("link `{graph}` uses wrong arrow direction: expected {expected}")]
    ArrowDirectionMismatch { graph: String, expected: String },
    #[error("acyclic link `{graph}` forms a cycle ({path}) during {interval}")]
    GraphCycle {
        graph: String,
        path: String,
        interval: String,
    },
    #[error("asymmetric link `{graph}` has bidirectional edges between `{a}` and `{b}` during {interval}")]
    AsymmetryConflict {
        graph: String,
        a: String,
        b: String,
        interval: String,
    },
    #[error("intransitive link `{graph}` has shortcut `{a} -> {c}` bypassing `{a} -> {b} -> {c}` during {interval}")]
    IntransitivityViolation {
        graph: String,
        a: String,
        b: String,
        c: String,
        interval: String,
    },
    #[error("functional link `{graph}` has multiple outgoing targets from `{node}` ({targets}) during {interval}")]
    FunctionalConflict {
        graph: String,
        node: String,
        targets: String,
        interval: String,
    },
    #[error("inverse_functional link `{graph}` has multiple incoming sources to `{node}` ({sources}) during {interval}")]
    InverseFunctionalConflict {
        graph: String,
        node: String,
        sources: String,
        interval: String,
    },
    #[error("link edge `{edge}` in `{graph}` ends before it starts")]
    LinkEndBeforeStart { graph: String, edge: String },
    #[error("event {event} refers to unknown entity {entity}")]
    UnknownEventEntity { event: String, entity: String },
    #[error("events {events} includes unknown event {event}")]
    UnknownEventsEvent { events: String, event: String },
    #[error("entity {entity} avatar must be an existing image file: {path}")]
    InvalidAvatarPath { entity: String, path: String },
    #[error("time must be a nonempty list")]
    TimeNonemptyList,
    #[error("time component must be an integer")]
    TimeIntegerComponent,
    #[error("time nesting exceeds maximum depth {maximum}")]
    TimeNestingTooDeep { maximum: String },
    #[error("event {event} ends before it starts")]
    EventEndBeforeStart { event: String },
    #[error("calendar path must point to an existing .rn file: {path}")]
    InvalidCalendarPath { path: String },
    #[error("calculable prop {prop} path must point to an existing .rn file: {path}")]
    InvalidCalculablePropPath { prop: String, path: String },
    #[error("failed to read calculable prop {prop}: {details}")]
    CalculablePropFileRead {
        prop: String,
        #[serde(rename = "source")]
        details: String,
    },
    #[error("failed to compile calculable prop {prop}: {details}")]
    CalculablePropCompile {
        prop: String,
        #[serde(rename = "source")]
        details: String,
    },
    #[error("calculable prop {prop} is missing public interfaces: {functions}")]
    CalculablePropMissingInterface { prop: String, functions: String },
    #[error("unknown calculable prop: {prop}")]
    UnknownCalculableProp { prop: String },
    #[error("failed to convert arguments for calculable prop {prop}: {details}")]
    CalculablePropArgument {
        prop: String,
        #[serde(rename = "source")]
        details: String,
    },
    #[error("failed to call {entry} in calculable prop {prop}: {details}")]
    CalculablePropCall {
        prop: String,
        entry: String,
        #[serde(rename = "source")]
        details: String,
    },
    #[error("invalid return value from {entry} in calculable prop {prop}; expected {expected}")]
    CalculablePropInvalidReturn {
        prop: String,
        entry: String,
        expected: String,
    },
    #[error("calendar {calendar} component count is out of range")]
    CalendarComponentsOutOfRange { calendar: String },
    #[error("failed to read calendar {calendar}: {details}")]
    CalendarFileRead {
        calendar: String,
        #[serde(rename = "source")]
        details: String,
    },
    #[error("failed to compile calendar {calendar}: {details}")]
    CalendarCompile {
        calendar: String,
        #[serde(rename = "source")]
        details: String,
    },
    #[error("calendar {calendar} is missing public interfaces: {functions}")]
    CalendarMissingInterface { calendar: String, functions: String },
    #[error("failed to call {entry} in calendar {calendar}: {details}")]
    CalendarCall {
        calendar: String,
        entry: String,
        #[serde(rename = "source")]
        details: String,
    },
    #[error("invalid return value from {entry} in calendar {calendar}")]
    CalendarInvalidReturn { calendar: String, entry: String },
    #[error("too many time components: {actual}, maximum {maximum}")]
    TooManyTimeComponents { actual: String, maximum: String },
    #[error("normalize returned the wrong number of components; expected {expected}")]
    NormalizeWrongComponentCount { expected: String },
    #[error("time offset is out of range")]
    TimeOffsetOutOfRange,
    #[error("time component {position} is {value}, below the calendar minimum {minimum}")]
    TimeComponentBelowMinimum {
        position: String,
        value: String,
        minimum: String,
    },
    #[error("calendar {calendar}: to_tick(from_tick(tick)) does not return the same tick")]
    CalendarRoundTripMismatch { calendar: String },
}

impl ValidationError {
    /// 面向 CLI / 配置文件的稳定 kebab-case 规则名。
    pub fn rule(&self) -> &'static str {
        match self {
            Self::SyntaxError { .. } => "syntax-error",
            Self::MissingStory => "missing-story",
            Self::DuplicateStory => "duplicate-story",
            Self::DuplicateEntity { .. } => "duplicate-entity",
            Self::DuplicateEvent { .. } => "duplicate-event",
            Self::DuplicateLinkGraph { .. } => "duplicate-link-graph",
            Self::DuplicateSymbol { .. } => "duplicate-symbol",
            Self::DuplicateEvents { .. } => "duplicate-events",
            Self::DuplicateTimeAnchor { .. } => "duplicate-time-anchor",
            Self::InvalidGraphConfig { .. } => "invalid-graph-config",
            Self::MissingField { .. } => "missing-field",
            Self::InvalidField { .. } => "invalid-field",
            Self::UnknownField { .. } => "unknown-field",
            Self::UnknownTimeRef { .. } => "unknown-time-ref",
            Self::TimeCycle { .. } => "time-cycle",
            Self::UnknownLinkEntity { .. } => "unknown-link-entity",
            Self::MixedLinkNodeKinds { .. } => "mixed-link-node-kinds",
            Self::EventGraphEquivalence { .. } => "event-graph-equivalence",
            Self::EventLinkHasTimeBounds { .. } => "event-link-time-bounds",
            Self::EquivalenceLinkHasTimeBounds { .. } => "equivalence-link-time-bounds",
            Self::ChronologicalViolation { .. } => "chronological-violation",
            Self::InvalidClique { .. } => "invalid-clique",
            Self::EmptyLinkEndpoint { .. } => "empty-link-endpoint",
            Self::ArrowDirectionMismatch { .. } => "arrow-direction-mismatch",
            Self::GraphCycle { .. } => "graph-cycle",
            Self::AsymmetryConflict { .. } => "asymmetry-conflict",
            Self::IntransitivityViolation { .. } => "intransitivity-violation",
            Self::FunctionalConflict { .. } => "functional-conflict",
            Self::InverseFunctionalConflict { .. } => "inverse-functional-conflict",
            Self::LinkEndBeforeStart { .. } => "link-end-before-start",
            Self::UnknownEventEntity { .. } => "unknown-event-entity",
            Self::UnknownEventsEvent { .. } => "unknown-events-event",
            Self::InvalidAvatarPath { .. } => "invalid-avatar-path",
            Self::TimeNonemptyList => "time-nonempty-list",
            Self::TimeIntegerComponent => "time-integer-component",
            Self::TimeNestingTooDeep { .. } => "time-nesting-too-deep",
            Self::EventEndBeforeStart { .. } => "event-end-before-start",
            Self::InvalidCalendarPath { .. } => "invalid-calendar-path",
            Self::InvalidCalculablePropPath { .. } => "invalid-calculable-prop-path",
            Self::CalculablePropFileRead { .. } => "calculable-prop-file-read",
            Self::CalculablePropCompile { .. } => "calculable-prop-compile",
            Self::CalculablePropMissingInterface { .. } => "calculable-prop-missing-interface",
            Self::UnknownCalculableProp { .. } => "unknown-calculable-prop",
            Self::CalculablePropArgument { .. } => "calculable-prop-argument",
            Self::CalculablePropCall { .. } => "calculable-prop-call",
            Self::CalculablePropInvalidReturn { .. } => "calculable-prop-invalid-return",
            Self::CalendarComponentsOutOfRange { .. } => "calendar-components-out-of-range",
            Self::CalendarFileRead { .. } => "calendar-file-read",
            Self::CalendarCompile { .. } => "calendar-compile",
            Self::CalendarMissingInterface { .. } => "calendar-missing-interface",
            Self::CalendarCall { .. } => "calendar-call",
            Self::CalendarInvalidReturn { .. } => "calendar-invalid-return",
            Self::TooManyTimeComponents { .. } => "too-many-time-components",
            Self::NormalizeWrongComponentCount { .. } => "normalize-wrong-component-count",
            Self::TimeOffsetOutOfRange => "time-offset-out-of-range",
            Self::TimeComponentBelowMinimum { .. } => "time-component-below-minimum",
            Self::CalendarRoundTripMismatch { .. } => "calendar-round-trip-mismatch",
        }
    }

    /// 按语言（`zh` / `en`）渲染面向用户的错误消息，作为 CLI、LSP 与桌面端的单一事实来源。
    pub fn message_for(&self, lang: &str) -> String {
        if lang != "zh" {
            return self.to_string();
        }
        match self {
            Self::SyntaxError { details } => format!("语法错误：{details}"),
            Self::MissingStory => "缺少必填的 story 块".to_owned(),
            Self::DuplicateStory => "重复声明了 story 块".to_owned(),
            Self::DuplicateEntity { entity } => format!("重复声明了实体 `{entity}`"),
            Self::DuplicateEvent { event } => format!("重复声明了事件 `{event}`"),
            Self::DuplicateLinkGraph { graph } => format!("重复声明了关系图 `{graph}`"),
            Self::DuplicateSymbol {
                name,
                existing_kind,
                new_kind,
            } => format!("节点名 `{name}` 已被声明为 {existing_kind}，不能再声明为 {new_kind}"),
            Self::DuplicateEvents { events } => format!("重复声明了事件列表 `{events}`"),
            Self::DuplicateTimeAnchor { name } => format!("重复声明了时间锚点 `{name}`"),
            Self::InvalidGraphConfig {
                graph,
                attr_a,
                attr_b,
            } => format!("关系图 `{graph}` 同时启用了互斥属性 `{attr_a}` 与 `{attr_b}`"),
            Self::MissingField { section, field } => {
                format!("{section} 缺少必填字段 `{field}`")
            }
            Self::InvalidField {
                section,
                field,
                reason,
            } => format!("{section} 的字段 `{field}` 值无效：{reason}"),
            Self::UnknownField { section, field } => {
                format!("{section} 包含未知字段 `{field}`")
            }
            Self::UnknownTimeRef { reference } => format!("引用了不存在的时间 `{reference}`"),
            Self::TimeCycle { target } => format!("时间引用 `{target}` 形成了循环"),
            Self::UnknownLinkEntity { graph, entity } => {
                format!("link `{graph}` 引用了不存在的实体或事件 `{entity}`")
            }
            Self::MixedLinkNodeKinds {
                graph,
                expected_kind,
                found_kind,
                node,
            } => {
                format!(
                    "关系图 `{graph}` 混用了 {expected_kind} 与 {found_kind} 节点（`{node}`）；同一张 link 图只能全连 entity 或全连 event"
                )
            }
            Self::EventGraphEquivalence { graph } => {
                format!("事件关系图 `{graph}` 不能启用 `equivalence`（商图等价仅适用于 entity）")
            }
            Self::EventLinkHasTimeBounds { graph } => {
                format!(
                    "事件关系图 `{graph}` 的边不能指定 `start` 或 `end`（事件自身已具备时间坐标）"
                )
            }
            Self::EquivalenceLinkHasTimeBounds { graph } => {
                format!("等价关系图 `{graph}` 的边不能指定 `start` 或 `end`")
            }
            Self::ChronologicalViolation {
                graph,
                from,
                from_time,
                to,
                to_time,
            } => {
                format!(
                    "时序关系图 `{graph}` 出现时间穿帮：前置节点 `{from}`（{from_time}）晚于后续节点 `{to}`（{to_time}）"
                )
            }
            Self::InvalidClique { graph, count } => {
                format!("link `{graph}` 中的团（Clique）至少需要 2 个节点，当前只有 {count} 个")
            }
            Self::EmptyLinkEndpoint { graph } => {
                format!("link `{graph}` 中边的端点列表不能为空")
            }
            Self::ArrowDirectionMismatch { graph, expected } => {
                format!("link `{graph}` 中的边方向与图约束不匹配，应使用 {expected}")
            }
            Self::GraphCycle {
                graph,
                path,
                interval,
            } => format!("无环图 `{graph}` 在时段 {interval} 内形成有向环：{path}"),
            Self::AsymmetryConflict {
                graph,
                a,
                b,
                interval,
            } => format!("非对称图 `{graph}` 在时段 {interval} 内出现双向互指：`{a}` ↔ `{b}`"),
            Self::IntransitivityViolation {
                graph,
                a,
                b,
                c,
                interval,
            } => format!(
                "反传递图 `{graph}` 在时段 {interval} 内存在跳级直连 `{a} -> {c}`（已有 `{a} -> {b} -> {c}`）"
            ),
            Self::FunctionalConflict {
                graph,
                node,
                targets,
                interval,
            } => format!(
                "多对一图 `{graph}` 在时段 {interval} 内节点 `{node}` 出度超过 1（指向：{targets}）"
            ),
            Self::InverseFunctionalConflict {
                graph,
                node,
                sources,
                interval,
            } => format!(
                "一对多图 `{graph}` 在时段 {interval} 内节点 `{node}` 入度超过 1（来自：{sources}）"
            ),
            Self::LinkEndBeforeStart { graph, edge } => {
                format!("link `{graph}` 的边 `{edge}` 结束时间早于开始时间")
            }
            Self::UnknownEventEntity { event, entity } => {
                format!("event `{event}` 引用了不存在的实体 `{entity}`")
            }
            Self::UnknownEventsEvent { events, event } => {
                format!("事件列表 `{events}` 引用了不存在的事件 `{event}`")
            }
            Self::InvalidAvatarPath { entity, path } => {
                format!("entity `{entity}` 的 avatar `{path}` 必须指向已存在的图片文件")
            }
            Self::TimeNonemptyList => "时间必须是非空整数列表或有效时间表达式".to_owned(),
            Self::TimeIntegerComponent => "时间分量必须是整数".to_owned(),
            Self::TimeNestingTooDeep { maximum } => {
                format!("时间锚点嵌套超过最大深度 {maximum}")
            }
            Self::EventEndBeforeStart { event } => {
                format!("事件 `{event}` 的结束时间早于开始时间")
            }
            Self::InvalidCalendarPath { path } => {
                format!("历法路径 `{path}` 必须指向现有的 .rn 文件")
            }
            Self::InvalidCalculablePropPath { prop, path } => {
                format!("可计算属性 `{prop}` 的路径 `{path}` 必须指向现有的 .rn 文件")
            }
            Self::CalculablePropFileRead { prop, details } => {
                format!("读取可计算属性 `{prop}` 的脚本失败：{details}")
            }
            Self::CalculablePropCompile { prop, details } => {
                format!("可计算属性 `{prop}` 编译失败：{details}")
            }
            Self::CalculablePropMissingInterface { prop, functions } => {
                format!("可计算属性 `{prop}` 缺少公开接口：{functions}")
            }
            Self::UnknownCalculableProp { prop } => {
                format!("引用了未注册的可计算属性 `{prop}`")
            }
            Self::CalculablePropArgument { prop, details } => {
                format!("可计算属性 `{prop}` 入参转换失败：{details}")
            }
            Self::CalculablePropCall {
                prop,
                entry,
                details,
            } => format!("可计算属性 `{prop}` 的 `{entry}` 执行失败：{details}"),
            Self::CalculablePropInvalidReturn {
                prop,
                entry,
                expected,
            } => format!("可计算属性 `{prop}` 的 `{entry}` 返回值无效，预期 {expected}"),
            Self::CalendarComponentsOutOfRange { calendar } => {
                format!("历法 `{calendar}` 的分量数必须在 1 到 64 之间")
            }
            Self::CalendarFileRead { calendar, details } => {
                format!("读取历法 `{calendar}` 的脚本失败：{details}")
            }
            Self::CalendarCompile { calendar, details } => {
                format!("历法 `{calendar}` 编译失败：{details}")
            }
            Self::CalendarMissingInterface {
                calendar,
                functions,
            } => format!("历法 `{calendar}` 缺少公开接口：{functions}"),
            Self::CalendarCall {
                calendar,
                entry,
                details,
            } => format!("历法 `{calendar}` 的 `{entry}` 执行失败：{details}"),
            Self::CalendarInvalidReturn { calendar, entry } => {
                format!("历法 `{calendar}` 的 `{entry}` 返回值无效")
            }
            Self::TooManyTimeComponents { actual, maximum } => {
                format!("时间分量过多：{actual} 个，该历法最多 {maximum} 个")
            }
            Self::NormalizeWrongComponentCount { expected } => {
                format!("from_tick 必须返回 {expected} 个时间分量")
            }
            Self::TimeOffsetOutOfRange => "时间偏移超出整数范围".to_owned(),
            Self::TimeComponentBelowMinimum {
                position,
                value,
                minimum,
            } => format!("第 {position} 个时间分量为 {value}，低于该历法允许的最小值 {minimum}"),
            Self::CalendarRoundTripMismatch { calendar } => {
                format!("历法 `{calendar}` 的 to_tick(from_tick(tick)) 未能还原同一刻度")
            }
        }
    }
}

#[derive(Clone, Debug, Error, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DocumentError {
    #[error("{}", format_validation(error, span.as_ref()))]
    Validation {
        #[source]
        error: Box<ValidationError>,
        #[serde(skip_serializing_if = "Option::is_none")]
        span: Option<SourceSpan>,
    },
    #[error("{message}")]
    Raw { message: String },
}

fn format_validation(error: &ValidationError, span: Option<&SourceSpan>) -> String {
    match span {
        Some(span) => format!("line {}, col {}: {error}", span.line, span.column),
        None => error.to_string(),
    }
}

impl DocumentError {
    pub fn validation_at(error: ValidationError, span: SourceSpan) -> Self {
        Self::Validation {
            error: Box::new(error),
            span: Some(span),
        }
    }

    pub fn with_span(self, span: SourceSpan) -> Self {
        match self {
            Self::Validation { error, span: None } => Self::Validation {
                error,
                span: Some(span),
            },
            other => other,
        }
    }

    pub fn raw(message: impl fmt::Display) -> Self {
        Self::Raw {
            message: message.to_string(),
        }
    }

    pub(crate) fn into_diagnostic(self, fallback_span: Option<ByteSpan>) -> Diagnostic {
        match self {
            Self::Validation { error, .. } => Diagnostic::from_validation(*error, fallback_span),
            Self::Raw { message } => Diagnostic::from_validation(
                ValidationError::SyntaxError { details: message },
                fallback_span,
            ),
        }
    }
}

impl From<ValidationError> for DocumentError {
    fn from(error: ValidationError) -> Self {
        Self::Validation {
            error: Box::new(error),
            span: None,
        }
    }
}
