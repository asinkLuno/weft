use serde::Serialize;
use ts_rs::TS;

// 校验错误的唯一代码定义。Serde 和 ts-rs 共用这些变体名与参数字段。
#[derive(Clone, Debug, Serialize, TS)]
#[serde(tag = "code", content = "params", rename_all = "snake_case")]
pub enum ValidationError {
    FileDeleted,
    DocumentNotBound,
    InvalidPath,
    DuplicateEventTitle { title: String },
    UnknownLinkEntity { group: String, entity: String },
    UnknownEventEntity { event: String, entity: String },
    UnknownObserver { narrative: String, entity: String },
    UnknownSubjectEvent { narrative: String, event: String },
    EntityUnknownParent { entity: String, target: String },
    EntityCycle { entity: String },
    TimeNonemptyList,
    TimeIntegerComponent,
    EventEndBeforeStart { event: String },
    UnregisteredCalendar { calendar: String },
    InvalidPluginSpec { name: String, kind: String },
    CalendarComponentsNotPositiveInteger { calendar: String },
    CalendarComponentsOutOfRange { calendar: String },
    TooManyTimeComponents { actual: String, maximum: String },
    NormalizeWrongComponentCount { expected: String },
    TimeOffsetOutOfRange,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DocumentError {
    Validation { error: ValidationError },
    Raw { message: String },
}

impl DocumentError {
    pub fn validation(error: ValidationError) -> Self {
        Self::Validation { error }
    }

    pub fn raw(message: impl ToString) -> Self {
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
