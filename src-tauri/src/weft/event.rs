//! 事件字典：键为事件 ID，值为解析后的事件。

use crate::calendar::Calendar;
use crate::weft::entity::Entity;
use crate::weft::errors::{DocumentError, ValidationError};
use crate::weft::phase::Phase;
use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;
use std::collections::{BTreeMap, HashMap};

// YAML 输入；时间保留原始值，构建 Event 时经 Phase::from_yaml_scalar 解析。
#[derive(Debug, Deserialize)]
pub(crate) struct RawEvent {
    pub start_time: Value,
    pub end_time: Option<Value>,
    #[serde(default)]
    pub entities: Vec<String>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Event {
    pub start_time: Phase,
    pub end_time: Option<Phase>,
    pub description: Option<String>,
    // 参与实体 ID，引用由 check 校验。
    pub entities: Vec<String>,
    // 展示层的计算结果。
    pub start_tick: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_tick: Option<i64>,
    pub start_display: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_display: Option<String>,
}

pub(crate) fn build_events(
    events: HashMap<String, RawEvent>,
) -> Result<BTreeMap<String, Event>, DocumentError> {
    events
        .into_iter()
        .map(|(id, raw)| {
            let event = Event {
                start_time: Phase::from_yaml_scalar(&raw.start_time)?,
                end_time: Phase::from_yaml_scalar_opt(raw.end_time.as_ref())?,
                description: raw.description,
                entities: raw.entities,
                start_tick: None,
                end_tick: None,
                start_display: None,
                end_display: None,
            };
            Ok((id, event))
        })
        .collect()
}

pub(crate) fn check(
    events: &HashMap<String, RawEvent>,
    entities: &HashMap<String, Entity>,
) -> Result<(), DocumentError> {
    let mut ids: Vec<_> = events.keys().collect();
    ids.sort_unstable();
    for id in ids {
        for entity in &events[id].entities {
            if !entities.contains_key(entity) {
                return Err(ValidationError::UnknownEventEntity {
                    event: id.clone(),
                    entity: entity.clone(),
                }
                .into());
            }
        }
    }
    Ok(())
}

pub(crate) fn apply_times(
    event: &mut BTreeMap<String, Event>,
    cal: &Calendar,
) -> Result<(), DocumentError> {
    for (id, ev) in event {
        let start = cal.resolve(&ev.start_time)?;
        let end = ev
            .end_time
            .as_ref()
            .map(|phase| cal.resolve(phase))
            .transpose()?;
        if end.as_ref().is_some_and(|end| end.tick < start.tick) {
            return Err(ValidationError::EventEndBeforeStart { event: id.clone() }.into());
        }
        ev.start_display = Some(cal.humanize(&start)?);
        ev.end_display = end.as_ref().map(|end| cal.humanize(end)).transpose()?;
        ev.start_tick = Some(start.tick);
        ev.end_tick = end.map(|end| end.tick);
    }
    Ok(())
}
