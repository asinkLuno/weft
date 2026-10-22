use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use crate::calendar::Calendar;
use crate::weft::entity::{Entity, resolve_base};
use crate::weft::errors::{DocumentError, ValidationError};
use crate::weft::event::{EventIndex, EventItem, resolve_event};

// 叙事线：subject 事件按 observer 的 base_time 展开成相对时间轴（与 event 的绝对时间轴相反）
#[derive(Debug, Serialize)]
pub struct Narrative {
    // 观察者，原样保留 YAML 引用（实体 id）
    pub observer: String,
    // 按 apply_times 填充的相对开始时间排序
    pub subject: Vec<NarrativeSubject>,
}

// subject 事件：event 为事件 id（`组名/标题`），tick 与展示均为相对 observer base 的偏移
#[derive(Debug, Serialize)]
pub struct NarrativeSubject {
    pub event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_tick: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_tick: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
}

// YAML 里的叙事线原始形态：subject 引用事件、observer 引用实体，校验后重写成 Narrative
#[derive(Debug, Deserialize)]
pub(crate) struct RawNarrative {
    subject: Vec<String>,
    observer: String,
}

// narrative 段检查：observer 与 subject 的引用目标存在。
// 引用错误批量收集，一次报出该段全部断链
pub(crate) fn check(
    narrative: &HashMap<String, RawNarrative>,
    entity: &HashMap<String, Entity>,
    index: &EventIndex<'_>,
) -> Result<(), DocumentError> {
    let mut errors = Vec::new();
    for (name, nar) in narrative {
        if !entity.contains_key(&nar.observer) {
            errors.push(ValidationError::UnknownObserver {
                narrative: name.clone(),
                entity: nar.observer.clone(),
            });
        }
        for r in &nar.subject {
            if resolve_event(index, r).is_none() {
                errors.push(ValidationError::UnknownSubjectEvent {
                    narrative: name.clone(),
                    event: r.clone(),
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

// 引用已在 check 校验，这里把 subject 重写成事件 id。
// 只依赖标题索引，单条 narrative 给索引即可单独解析
pub(crate) fn resolve_narratives(
    narrative: HashMap<String, RawNarrative>,
    index: &EventIndex<'_>,
) -> BTreeMap<String, Narrative> {
    let mut out = BTreeMap::new();
    for (name, nar) in narrative {
        let mut subject = Vec::with_capacity(nar.subject.len());
        for r in nar.subject {
            let (g, t) = resolve_event(index, &r).expect("subject 引用已校验");
            subject.push(NarrativeSubject {
                event: format!("{g}/{t}"),
                start_tick: None,
                end_tick: None,
                start: None,
                end: None,
            });
        }
        out.insert(
            name,
            Narrative {
                observer: nar.observer,
                subject,
            },
        );
    }
    out
}

// 叙事线：subject 事件相对 observer base_time 的偏移（tick 定位 + humanize 展示）。
// subject.event 在 model 层已重写成 `组名/标题` 全 id，这里按 id 直接取事件
pub(crate) fn apply_times(
    narrative: &mut BTreeMap<String, Narrative>,
    entity: &HashMap<String, Entity>,
    event: &BTreeMap<String, Vec<EventItem>>,
    cal: &Calendar,
) -> Result<(), DocumentError> {
    let by_id: HashMap<&str, &EventItem> = event
        .values()
        .flatten()
        .map(|ev| (ev.id.as_str(), ev))
        .collect();
    for nar in narrative.values_mut() {
        let base = resolve_base(entity, &nar.observer);
        let base_tick = base
            .map(|base| {
                let normalized = cal.normalize(&base.flatten(cal.component_count())?)?;
                cal.to_tick(&normalized)
            })
            .transpose()?;
        for subject in &mut nar.subject {
            let Some(ev) = by_id.get(subject.event.as_str()) else {
                continue;
            };
            subject.start_tick = offset_tick(base_tick, ev.start_tick)?;
            subject.end_tick = offset_tick(base_tick, ev.end_tick)?;
            subject.start = base
                .map(|base| cal.offset_display(&ev.start_phase, base))
                .transpose()?;
            subject.end = match (base, &ev.end_phase) {
                (Some(base), Some(time)) => Some(cal.offset_display(time, base)?),
                _ => None,
            };
        }
        nar.subject
            .sort_by_key(|s| s.start_tick.unwrap_or(i64::MAX));
    }
    Ok(())
}

// 两个 tick 相减；任一端没有时间就得到 None
fn offset_tick(base: Option<i64>, time: Option<i64>) -> Result<Option<i64>, DocumentError> {
    base.zip(time)
        .map(|(base, time)| {
            time.checked_sub(base)
                .ok_or(ValidationError::TimeOffsetOutOfRange)
        })
        .transpose()
        .map_err(DocumentError::from)
}
