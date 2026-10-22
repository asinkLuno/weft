use serde::Serialize;
use thiserror::Error;
use ts_rs::TS;

// 校验错误的唯一代码定义。Serde 和 ts-rs 共用这些变体名与参数字段。
#[derive(Clone, Debug, Error, Serialize, TS)]
#[serde(tag = "code", content = "params", rename_all = "snake_case")]
pub enum ValidationError {
    #[error("file deleted or moved")]
    FileDeleted,
    #[error("no document is bound")]
    DocumentNotBound,
    #[error("invalid path")]
    InvalidPath,
    #[error("duplicate event title: {title} in {groups}")]
    DuplicateEventTitle { title: String, groups: String },
    #[error("entity link {group} refers to unknown entity {entity}")]
    UnknownLinkEntity { group: String, entity: String },
    #[error("event {event} refers to unknown entity {entity}")]
    UnknownEventEntity { event: String, entity: String },
    #[error("narrative {narrative} refers to unknown observer {entity}")]
    UnknownObserver { narrative: String, entity: String },
    #[error("narrative {narrative} refers to unknown event {event}")]
    UnknownSubjectEvent { narrative: String, event: String },
    #[error("entity {entity} refers to unknown parent {target}")]
    EntityUnknownParent { entity: String, target: String },
    #[error("entity {entity} has a parent cycle")]
    EntityCycle { entity: String },
    #[error("time must be a nonempty list")]
    TimeNonemptyList,
    #[error("time component must be an integer")]
    TimeIntegerComponent,
    #[error("event {event} ends before it starts")]
    EventEndBeforeStart { event: String },
    #[error("calendar path must point to an existing .rhai file: {path}")]
    InvalidCalendarPath { path: String },
    #[error("calculable tag {tag} path must point to a .rhai file: {path}")]
    InvalidCalculableTagPath { tag: String, path: String },
    #[error("failed to read calculable tag {tag}: {details}")]
    CalculableTagFileRead {
        tag: String,
        #[serde(rename = "source")]
        details: String,
    },
    #[error("failed to compile calculable tag {tag}: {details}")]
    CalculableTagCompile {
        tag: String,
        #[serde(rename = "source")]
        details: String,
    },
    #[error("calculable tag {tag} is missing public interfaces: {functions}")]
    CalculableTagMissingInterface { tag: String, functions: String },
    #[error("unknown calculable tag: {tag}")]
    UnknownCalculableTag { tag: String },
    #[error("failed to convert arguments for calculable tag {tag}: {details}")]
    CalculableTagArgument {
        tag: String,
        #[serde(rename = "source")]
        details: String,
    },
    #[error("failed to call {entry} in calculable tag {tag}: {details}")]
    CalculableTagCall {
        tag: String,
        entry: String,
        #[serde(rename = "source")]
        details: String,
    },
    #[error("invalid return value from {entry} in calculable tag {tag}; expected {expected}")]
    CalculableTagInvalidReturn {
        tag: String,
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
}

#[derive(Clone, Debug, Error, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DocumentError {
    #[error("{error}")]
    Validation {
        #[source]
        error: ValidationError,
    },
    #[error("invalid references: {errors:?}")]
    References { errors: Vec<ValidationError> },
    #[error("{message}")]
    Raw { message: String },
}

impl DocumentError {
    pub(crate) fn validation(error: ValidationError) -> Self {
        Self::Validation { error }
    }

    pub(crate) fn raw(message: impl ToString) -> Self {
        Self::Raw {
            message: message.to_string(),
        }
    }
}

impl From<ValidationError> for DocumentError {
    fn from(error: ValidationError) -> Self {
        Self::validation(error)
    }
}

impl From<String> for DocumentError {
    fn from(message: String) -> Self {
        Self::raw(message)
    }
}

impl From<&str> for DocumentError {
    fn from(message: &str) -> Self {
        Self::raw(message)
    }
}
