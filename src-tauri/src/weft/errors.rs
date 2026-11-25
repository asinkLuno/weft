use std::fmt;

use serde::Serialize;
use thiserror::Error;
use ts_rs::TS;

/// 源码定位信息（UTF-16 偏移对齐 CodeMirror；1-based 行列号仅用于后端日志）。
#[derive(Clone, Debug, Serialize, TS)]
pub struct SourceSpan {
    pub start: usize,
    pub end: usize,
    #[serde(skip)]
    pub line: usize,
    #[serde(skip)]
    pub column: usize,
}

/// 交付给编辑器的单条编译诊断。
#[derive(Clone, Debug, Serialize, TS)]
pub struct CompileDiagnostic {
    pub error: ValidationError,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub span: Option<SourceSpan>,
}

// 校验错误的唯一代码定义。Serde 和 ts-rs 共用这些变体名与参数字段。
#[derive(Clone, Debug, Error, Serialize, TS)]
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
    #[error("duplicate narrative `{narrative}`")]
    DuplicateNarrative { narrative: String },
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
    #[error("narrative {narrative} refers to unknown event {event}")]
    UnknownNarrativeEvent { narrative: String, event: String },
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
    pub(crate) fn validation_at(error: ValidationError, span: SourceSpan) -> Self {
        Self::Validation {
            error,
            span: Some(span),
        }
    }

    pub(crate) fn with_span(self, span: SourceSpan) -> Self {
        match self {
            Self::Validation { error, span: None } => Self::Validation {
                error,
                span: Some(span),
            },
            other => other,
        }
    }

    pub(crate) fn raw(message: impl fmt::Display) -> Self {
        Self::Raw {
            message: message.to_string(),
        }
    }
}

impl From<ValidationError> for DocumentError {
    fn from(error: ValidationError) -> Self {
        Self::Validation { error, span: None }
    }
}
