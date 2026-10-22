use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;

use crate::calendar::Calendar;
use crate::weft::entity::{Entity, resolve_base};
use crate::weft::errors::{DocumentError, ValidationError};
use crate::weft::phase::Phase;

// YAML 里的事件原始形态，解析成 EventItem 要等历法加载
#[derive(Debug, Deserialize)]
pub(crate) struct RawEvent {
    start_time: Value,
    end_time: Option<Value>,
    #[serde(default)]
    entities: Vec<String>,
    description: Option<String>,
}

// 事件：按组组织，id = `组名/标题`
#[derive(Debug, Serialize)]
pub struct EventItem {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    // 参与实体，原样保留 YAML 引用（实体 id）
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entities: Vec<String>,
    // 每个参与实体相对其 base_time 的偏移（apply_times 填充）
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub offsets: Vec<EntityOffset>,
    // 原始 phase 由 build_events 解析，时间结果由 apply_times 在加载历法后填充
    #[serde(skip)]
    pub start_phase: Phase,
    #[serde(skip)]
    pub end_phase: Option<Phase>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<Vec<i64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<Vec<i64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_tick: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_tick: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_display: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_display: Option<String>,
}

// 参与者相对其 base_time 的偏移展示；start/end 为 None 表示该实体没有 base_time
#[derive(Debug, Serialize)]
pub struct EntityOffset {
    pub entity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
}

// 事件 id = `组名/标题`；时间计算要等历法加载后调用 apply_times
pub(crate) fn build_events(
    event: HashMap<String, HashMap<String, RawEvent>>,
) -> Result<BTreeMap<String, Vec<EventItem>>, DocumentError> {
    let mut out = BTreeMap::new();
    for (group, events) in event {
        let mut list = Vec::with_capacity(events.len());
        for (title, ev) in events {
            list.push(EventItem {
                id: format!("{group}/{title}"),
                title,
                description: ev.description,
                entities: ev.entities,
                offsets: Vec::new(),
                start_phase: Phase::from_yaml(&ev.start_time)?,
                end_phase: ev.end_time.as_ref().map(Phase::from_yaml).transpose()?,
                start: None,
                end: None,
                start_tick: None,
                end_tick: None,
                start_display: None,
                end_display: None,
            });
        }
        out.insert(group, list);
    }
    Ok(out)
}

// 事件标题索引：标题 → 组名列表（组名已排序，查重报告顺序稳定）。
// 裸标题引用靠它定位组名，`组名/标题` 全 id 引用靠它验证存在；
// 不含事件体，单条引用拿索引即可单独解析
pub(crate) type EventIndex<'a> = BTreeMap<&'a str, Vec<&'a str>>;

pub(crate) fn build_event_index(
    events: &HashMap<String, HashMap<String, RawEvent>>,
) -> EventIndex<'_> {
    let mut by_title: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (group, evs) in events {
        for title in evs.keys() {
            by_title.entry(title).or_default().push(group);
        }
    }
    for groups in by_title.values_mut() {
        groups.sort_unstable();
    }
    by_title
}

// 事件引用：`组名/标题` 全 id，或裸标题（须先过全局唯一查重才能定组）。
// 只返回借用，校验时不必为每条引用分配字符串
pub(crate) fn resolve_event<'a>(
    index: &'a EventIndex<'a>,
    r: &'a str,
) -> Option<(&'a str, &'a str)> {
    match r.split_once('/') {
        Some((group, title)) => index
            .get(title)
            .is_some_and(|groups| groups.contains(&group))
            .then_some((group, title)),
        None => index.get(r).map(|groups| (groups[0], r)),
    }
}

// event 段检查：先引用（参与实体存在）后结构（标题全局唯一，跨组同名也算重复，
// narrative 等引用才能省略组名）。引用错误批量收集，一次报出该段全部断链
pub(crate) fn check(
    events: &HashMap<String, HashMap<String, RawEvent>>,
    entity: &HashMap<String, Entity>,
    index: &EventIndex<'_>,
) -> Result<(), DocumentError> {
    let mut errors = Vec::new();
    for (group, evs) in events {
        for (title, ev) in evs {
            for r in &ev.entities {
                if !entity.contains_key(r) {
                    errors.push(ValidationError::UnknownEventEntity {
                        event: format!("{group}/{title}"),
                        entity: r.clone(),
                    });
                }
            }
        }
    }
    if !errors.is_empty() {
        errors.sort_by_cached_key(ToString::to_string);
        return Err(DocumentError::References { errors });
    }
    check_unique_event_titles(index)
}

// 组名列表建索引时已排序：报哪个标题、组名顺序都不随 HashMap 迭代漂移
fn check_unique_event_titles(index: &EventIndex<'_>) -> Result<(), DocumentError> {
    if let Some((title, groups)) = index.iter().find(|(_, groups)| groups.len() > 1) {
        return Err(ValidationError::DuplicateEventTitle {
            title: (*title).to_string(),
            groups: groups.join(", "),
        }
        .into());
    }
    Ok(())
}

// 事件：phase 递归展平后由历法归一化为绝对分量→ tick（排序与先后校验的基准）→ humanize，
// 同时算出每个参与实体相对其 base_time 的偏移
pub(crate) fn apply_times(
    event: &mut BTreeMap<String, Vec<EventItem>>,
    entity: &HashMap<String, Entity>,
    cal: &Calendar,
) -> Result<(), DocumentError> {
    for events in event.values_mut() {
        for ev in &mut *events {
            let start = cal.normalize(&ev.start_phase.flatten(cal.component_count())?)?;
            let start_tick = cal.to_tick(&start)?;
            let end = ev
                .end_phase
                .as_ref()
                .map(|phase| cal.normalize(&phase.flatten(cal.component_count())?))
                .transpose()?;
            let end_tick = end.as_ref().map(|end| cal.to_tick(end)).transpose()?;
            if end_tick.is_some_and(|end| end < start_tick) {
                return Err(ValidationError::EventEndBeforeStart {
                    event: ev.id.clone(),
                }
                .into());
            }
            let start_display = cal.humanize(&start)?;
            let end_display = end.as_ref().map(|end| cal.humanize(end)).transpose()?;
            // 没有 base_time 的实体 start 留空
            ev.offsets = ev
                .entities
                .iter()
                .map(|r| {
                    let base = resolve_base(entity, r);
                    Ok(EntityOffset {
                        entity: r.clone(),
                        start: base
                            .map(|base| cal.offset_display(&ev.start_phase, base))
                            .transpose()?,
                        end: match (base, &ev.end_phase) {
                            (Some(base), Some(end)) => cal.offset_display(end, base).map(Some)?,
                            _ => None,
                        },
                    })
                })
                .collect::<Result<_, DocumentError>>()?;
            ev.start = Some(start);
            ev.end = end;
            ev.start_tick = Some(start_tick);
            ev.end_tick = end_tick;
            ev.start_display = Some(start_display);
            ev.end_display = end_display;
        }
        events.sort_by_key(|e| e.start_tick.unwrap_or(i64::MAX));
    }
    Ok(())
}
