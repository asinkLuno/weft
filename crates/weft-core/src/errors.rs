//! 统一源码区间、编译校验错误与多诊断模型。

use std::{fmt, ops::Range};

use serde::Serialize;
use thiserror::Error;
#[cfg(feature = "ts-export")]
use ts_rs::TS;

/// UTF-8 字节区间（语言内核唯一位置真相；终端渲染和编辑器按需换算行列或 UTF-16）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
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
#[cfg_attr(feature = "ts-export", derive(TS))]
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
#[cfg_attr(feature = "ts-export", derive(TS))]
pub struct CompileDiagnostic {
    pub error: ValidationError,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
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

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }

    pub fn to_compile_diagnostic(&self, src: &str) -> Option<CompileDiagnostic> {
        let error = self.error.clone()?;
        let span = self.span.map(|s| s.to_source_span(src));
        Some(CompileDiagnostic { error, span })
    }
}

// 校验错误的唯一代码定义。Serde 和 ts-rs 共用这些变体名与参数字段。
#[derive(Clone, Debug, Error, Serialize)]
#[cfg_attr(feature = "ts-export", derive(TS))]
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
    #[error("duplicate events `{events}`")]
    DuplicateEvents { events: String },
    #[error("duplicate time anchor `{name}`")]
    DuplicateTimeAnchor { name: String },
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
    #[error("entity link {group} refers to unknown entity {entity}")]
    UnknownLinkEntity { group: String, entity: String },
    #[error("event {event} refers to unknown entity {entity}")]
    UnknownEventEntity { event: String, entity: String },
    #[error("events {events} includes unknown event {event}")]
    UnknownEventsEvent { events: String, event: String },
    #[error("entity {entity} refers to unknown parent {target}")]
    EntityUnknownParent { entity: String, target: String },
    #[error("entity {entity} has a parent cycle")]
    EntityCycle { entity: String },
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
            Self::DuplicateEvents { .. } => "duplicate-events",
            Self::DuplicateTimeAnchor { .. } => "duplicate-time-anchor",
            Self::MissingField { .. } => "missing-field",
            Self::InvalidField { .. } => "invalid-field",
            Self::UnknownField { .. } => "unknown-field",
            Self::UnknownTimeRef { .. } => "unknown-time-ref",
            Self::TimeCycle { .. } => "time-cycle",
            Self::UnknownLinkEntity { .. } => "unknown-link-entity",
            Self::UnknownEventEntity { .. } => "unknown-event-entity",
            Self::UnknownEventsEvent { .. } => "unknown-events-event",
            Self::EntityUnknownParent { .. } => "entity-unknown-parent",
            Self::EntityCycle { .. } => "entity-cycle",
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
}

#[derive(Clone, Debug, Error, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DocumentError {
    #[error("{}", format_validation(error, span.as_ref()))]
    Validation {
        #[source]
        error: ValidationError,
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
            error,
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
            Self::Validation { error, .. } => Diagnostic::from_validation(error, fallback_span),
            Self::Raw { message } => Diagnostic::from_validation(
                ValidationError::SyntaxError { details: message },
                fallback_span,
            ),
        }
    }
}

impl From<ValidationError> for DocumentError {
    fn from(error: ValidationError) -> Self {
        Self::Validation { error, span: None }
    }
}
