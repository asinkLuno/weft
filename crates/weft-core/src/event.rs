//! 事件字典：键为事件 ID，值为解析后的事件。

use std::collections::{BTreeMap, HashMap};

use serde::Serialize;

use crate::calendar::{Calendar, Moment};
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

/// 事件起止时间相对某个参与实体 `base_time` 的差值。
#[derive(Debug, Serialize)]
pub struct EntityTimeDelta {
    pub start_tick: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_tick: Option<i64>,
    pub start_display: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_display: Option<String>,
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
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub entity_deltas: BTreeMap<String, EntityTimeDelta>,
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
                    diag = diag.with_suggestion(suggestion);
                }
                diagnostics.push(diag);
            }
        }
    }
}

/// 一步完成事件时间解析、先后校验、展示格式化与实体基准时间差计算，收集全部历法与起止时间错误。
pub(crate) fn build_events_collecting(
    events: BTreeMap<String, RawEvent>,
    entity_base_times: &HashMap<String, Moment>,
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
        let entity_deltas = compute_entity_deltas(
            &raw.entities,
            entity_base_times,
            &start,
            end.as_ref(),
            cal,
            diagnostics,
        );
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
                entity_deltas,
            },
        );
    }
    out
}

fn compute_entity_deltas(
    entities: &[(String, ByteSpan)],
    entity_base_times: &HashMap<String, Moment>,
    start: &Moment,
    end: Option<&Moment>,
    cal: &Calendar,
    diagnostics: &mut Vec<Diagnostic>,
) -> BTreeMap<String, EntityTimeDelta> {
    let mut deltas = BTreeMap::new();
    for (entity_id, entity_span) in entities {
        if deltas.contains_key(entity_id) {
            continue;
        }
        let Some(base_moment) = entity_base_times.get(entity_id) else {
            continue;
        };
        match cal.humanize_delta_pair(base_moment, start, end) {
            Ok((start_display, end_display)) => {
                deltas.insert(
                    entity_id.clone(),
                    EntityTimeDelta {
                        start_tick: start.tick.saturating_sub(base_moment.tick),
                        end_tick: end.map(|m| m.tick.saturating_sub(base_moment.tick)),
                        start_display,
                        end_display,
                    },
                );
            }
            Err(err) => {
                diagnostics.push(err.into_diagnostic(Some(*entity_span)));
            }
        }
    }
    deltas
}
