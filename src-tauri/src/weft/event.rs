//! 事件字典：键为事件 ID，值为解析后的事件。

use std::collections::{BTreeMap, HashMap};

use serde::Serialize;

use crate::calendar::Calendar;
use crate::weft::entity::Entity;
use crate::weft::errors::{DocumentError, SourceSpan, ValidationError};
use crate::weft::phase::Phase;

#[derive(Debug)]
pub(crate) struct RawEvent {
    pub start_time: (Phase, SourceSpan),
    pub end_time: Option<(Phase, SourceSpan)>,
    pub entities: Vec<(String, SourceSpan)>,
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

pub(crate) fn check(
    events: &BTreeMap<String, RawEvent>,
    entities: &HashMap<String, Entity>,
) -> Result<(), DocumentError> {
    for (id, raw) in events {
        for (entity, span) in &raw.entities {
            if !entities.contains_key(entity) {
                return Err(DocumentError::validation_at(
                    ValidationError::UnknownEventEntity {
                        event: id.clone(),
                        entity: entity.clone(),
                    },
                    span.clone(),
                ));
            }
        }
    }
    Ok(())
}

/// 一步完成事件时间解析、先后校验与展示格式化，保证输出的 `Event` 不含半初始化状态。
pub(crate) fn build_events(
    events: BTreeMap<String, RawEvent>,
    cal: &Calendar,
) -> Result<BTreeMap<String, Event>, DocumentError> {
    let mut out = BTreeMap::new();
    for (id, raw) in events {
        let (start_time, start_span) = raw.start_time;
        let start = cal
            .resolve(&start_time)
            .map_err(|err| err.with_span(start_span))?;
        let (end_time, end) = match raw.end_time {
            Some((phase, end_span)) => {
                let moment = cal
                    .resolve(&phase)
                    .map_err(|err| err.with_span(end_span.clone()))?;
                if moment.tick < start.tick {
                    return Err(DocumentError::validation_at(
                        ValidationError::EventEndBeforeStart { event: id },
                        end_span,
                    ));
                }
                (Some(phase), Some(moment))
            }
            None => (None, None),
        };
        let start_display = cal.humanize(&start)?;
        let end_display = end.as_ref().map(|m| cal.humanize(m)).transpose()?;
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
    Ok(out)
}
