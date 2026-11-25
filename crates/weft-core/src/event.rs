//! 事件字典：键为事件 ID，值为解析后的事件。

use std::collections::BTreeMap;

use serde::Serialize;

use crate::calendar::Calendar;
use crate::entity::RawEntity;
use crate::errors::{ByteSpan, Diagnostic, ValidationError};
use crate::lint::suggest_similar;
use crate::phase::Phase;

#[derive(Debug)]
pub(crate) struct RawEvent {
    pub name_span: ByteSpan,
    pub start_time: Option<(Phase, ByteSpan)>,
    pub end_time: Option<(Phase, ByteSpan)>,
    pub entities: Vec<(String, ByteSpan)>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Event {
    pub start_time: Phase,
    pub end_time: Option<Phase>,
    pub description: Option<String>,
    pub entities: Vec<String>,
    pub start_tick: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_tick: Option<i64>,
    pub start_display: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_display: Option<String>,
}

pub(crate) fn check_collecting(
    events: &BTreeMap<String, RawEvent>,
    entities: &BTreeMap<String, RawEntity>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let entity_names: Vec<&str> = entities.keys().map(String::as_str).collect();
    for (id, raw) in events {
        for (entity, span) in &raw.entities {
            if !entities.contains_key(entity) {
                let mut diag = Diagnostic::from_validation(
                    ValidationError::UnknownEventEntity {
                        event: id.clone(),
                        entity: entity.clone(),
                    },
                    Some(*span),
                );
                if let Some(suggestion) = suggest_similar(entity, &entity_names) {
                    diag = diag.with_help(format!("did you mean `{suggestion}`?"));
                }
                diagnostics.push(diag);
            }
        }
    }
}

/// 一步完成事件时间解析、先后校验与展示格式化，收集全部历法与起止时间错误。
pub(crate) fn build_events_collecting(
    events: BTreeMap<String, RawEvent>,
    cal: &Calendar,
    diagnostics: &mut Vec<Diagnostic>,
) -> BTreeMap<String, Event> {
    let mut out = BTreeMap::new();
    for (id, raw) in events {
        let Some((start_time, start_span)) = raw.start_time else {
            continue;
        };
        let start = match cal.resolve(&start_time) {
            Ok(moment) => moment,
            Err(err) => {
                diagnostics.push(err.into_diagnostic(Some(start_span)));
                continue;
            }
        };
        let (end_time, end) = match raw.end_time {
            Some((phase, end_span)) => match cal.resolve(&phase) {
                Ok(moment) => {
                    if moment.tick < start.tick {
                        diagnostics.push(
                            Diagnostic::from_validation(
                                ValidationError::EventEndBeforeStart { event: id.clone() },
                                Some(end_span),
                            )
                            .with_related(start_span, "event starts here"),
                        );
                        (Some(phase), None)
                    } else {
                        (Some(phase), Some(moment))
                    }
                }
                Err(err) => {
                    diagnostics.push(err.into_diagnostic(Some(end_span)));
                    (Some(phase), None)
                }
            },
            None => (None, None),
        };
        let start_display = match cal.humanize(&start) {
            Ok(text) => text,
            Err(err) => {
                diagnostics.push(err.into_diagnostic(Some(start_span)));
                continue;
            }
        };
        let end_display = match end.as_ref() {
            Some(moment) => match cal.humanize(moment) {
                Ok(text) => Some(text),
                Err(err) => {
                    diagnostics.push(err.into_diagnostic(None));
                    None
                }
            },
            None => None,
        };
        out.insert(
            id,
            Event {
                start_time,
                end_time,
                description: raw.description,
                entities: raw.entities.into_iter().map(|(entity, _)| entity).collect(),
                start_tick: start.tick,
                end_tick: end.map(|m| m.tick),
                start_display,
                end_display,
            },
        );
    }
    out
}
