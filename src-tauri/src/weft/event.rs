//! 事件字典：键为事件 ID，值为解析后的事件。

mod input;
pub(crate) use input::RawEvent;

use crate::calendar::Calendar;
use crate::weft::entity::Entity;
use crate::weft::errors::{DocumentError, ValidationError};
use crate::weft::phase::{Phase, offset_tick};
use serde::{Serialize, Serializer};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

#[derive(Debug, Serialize)]
pub struct EventItem {
    pub start_time: Phase,
    pub end_time: Option<Phase>,
    pub description: Option<String>,
    #[serde(serialize_with = "serialize_entities")]
    pub entities: Vec<Arc<Entity>>,
}

// 展示层的计算结果，不属于事件定义。
#[derive(Debug, Default, Serialize)]
pub struct EventTiming {
    pub offsets: Vec<EntityOffset>,
    pub start_tick: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_tick: Option<i64>,
    pub start_display: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_display: Option<String>,
}

fn serialize_entities<S: Serializer>(
    entities: &[Arc<Entity>],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    entities
        .iter()
        .map(|entity| &entity.id)
        .collect::<Vec<_>>()
        .serialize(serializer)
}

// 参与者相对 base_time 的 tick 偏移用于甘特图定位和长度；start/end 用于文本展示。
// 没有 base_time 时偏移留空；没有结束时间时结束偏移留空。
#[derive(Debug, Serialize)]
pub struct EntityOffset {
    pub entity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_tick: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_tick: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
}

pub(crate) fn build_events(
    events: HashMap<String, RawEvent>,
    entities: &HashMap<String, Arc<Entity>>,
) -> Result<BTreeMap<String, EventItem>, DocumentError> {
    events
        .into_iter()
        .map(|(id, raw)| {
            let event = EventItem {
                start_time: Phase::from_yaml(&raw.start_time)?,
                end_time: raw.end_time.as_ref().map(Phase::from_yaml).transpose()?,
                description: raw.description,
                // check 已校验所有实体引用。
                entities: raw
                    .entities
                    .iter()
                    .map(|id| Arc::clone(&entities[id]))
                    .collect(),
            };
            Ok((id, event))
        })
        .collect()
}

pub(crate) fn check(
    events: &HashMap<String, RawEvent>,
    entities: &HashMap<String, Arc<Entity>>,
) -> Result<(), DocumentError> {
    let mut errors = Vec::new();
    for (id, event) in events {
        for entity in &event.entities {
            if !entities.contains_key(entity) {
                errors.push(ValidationError::UnknownEventEntity {
                    event: id.clone(),
                    entity: entity.clone(),
                });
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        errors.sort_by_cached_key(ToString::to_string);
        Err(DocumentError::References { errors })
    }
}

pub(crate) fn apply_times(
    event: &BTreeMap<String, EventItem>,
    cal: &Calendar,
) -> Result<BTreeMap<String, EventTiming>, DocumentError> {
    let mut timings = BTreeMap::new();
    for (id, ev) in event {
        let mut timing = EventTiming::default();
        let start = cal.normalize(&ev.start_time.flatten(cal.component_count())?)?;
        let start_tick = cal.to_tick(&start)?;
        let end = ev
            .end_time
            .as_ref()
            .map(|phase| cal.normalize(&phase.flatten(cal.component_count())?))
            .transpose()?;
        let end_tick = end.as_ref().map(|end| cal.to_tick(end)).transpose()?;
        if end_tick.is_some_and(|end| end < start_tick) {
            return Err(ValidationError::EventEndBeforeStart { event: id.clone() }.into());
        }
        let start_display = cal.humanize(&start)?;
        let end_display = end.as_ref().map(|end| cal.humanize(end)).transpose()?;
        // 没有 base_time 的实体 start 留空
        timing.offsets = ev
            .entities
            .iter()
            .map(|entity| {
                let base = entity.base_time.as_ref();
                let base_tick = base
                    .map(|base| {
                        let normalized = cal.normalize(&base.flatten(cal.component_count())?)?;
                        cal.to_tick(&normalized)
                    })
                    .transpose()?;
                Ok(EntityOffset {
                    entity: entity.id.clone(),
                    start_tick: offset_tick(base_tick, Some(start_tick))?,
                    end_tick: offset_tick(base_tick, end_tick)?,
                    start: base
                        .map(|base| cal.offset_display(&ev.start_time, base))
                        .transpose()?,
                    end: match (base, &ev.end_time) {
                        (Some(base), Some(end)) => cal.offset_display(end, base).map(Some)?,
                        _ => None,
                    },
                })
            })
            .collect::<Result<_, DocumentError>>()?;
        timing.start_tick = Some(start_tick);
        timing.end_tick = end_tick;
        timing.start_display = Some(start_display);
        timing.end_display = end_display;
        timings.insert(id.clone(), timing);
    }
    Ok(timings)
}
