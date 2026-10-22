use std::collections::{BTreeMap, HashMap};

use serde::de::{self, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_yaml_ng::Value;

use crate::calendar::CalendarMetadata;
use crate::validation::{DocumentError, ValidationError};

// YAML 数据模型：顶层键逐个补齐，未知键在反序列化时滤掉。
#[derive(Debug, Serialize)]
pub struct WeftDoc {
    pub story: Story,
    pub entity: HashMap<String, Entity>,
    // 关系分组：组名 → 关系对列表
    #[serde(default)]
    pub entity_link: HashMap<String, Vec<EntityLink>>,
    // 自定义历法注册表：注册名 → Rhai 脚本清单（相对故事文件的路径）
    #[serde(default)]
    pub calendar: HashMap<String, CalendarSpec>,
    // 可计算标签注册表：注册名 → Rhai 脚本清单（与 calendar 同构），按实体 base_time 计算归属
    #[serde(default)]
    pub calculable_tag: HashMap<String, CalendarSpec>,
    // 事件分组：组名 → 按 tick 排序的事件列表（解析在 document.rs 完成）
    #[serde(default)]
    pub event: BTreeMap<String, Vec<EventItem>>,
    // 叙事线：组名 → 相对 observer base_time 展开的事件时间轴（解析在 document.rs 完成）
    #[serde(default)]
    pub narrative: BTreeMap<String, Narrative>,
    // 运行时填充：story.date_mode 所指历法的元数据，不来自 YAML
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calendar_meta: Option<CalendarMetadata>,
}

#[derive(Deserialize)]
struct RawWeftDoc {
    story: Story,
    #[serde(default)]
    entity: HashMap<String, Entity>,
    #[serde(default)]
    entity_link: HashMap<String, Vec<EntityLink>>,
    #[serde(default)]
    event: HashMap<String, HashMap<String, RawEvent>>,
    #[serde(default)]
    narrative: HashMap<String, RawNarrative>,
    #[serde(default)]
    calendar: HashMap<String, CalendarSpec>,
    #[serde(default)]
    calculable_tag: HashMap<String, CalendarSpec>,
}

impl WeftDoc {
    pub fn from_yaml(yaml: &Value) -> Result<Self, DocumentError> {
        let raw = RawWeftDoc::deserialize(yaml).map_err(DocumentError::raw)?;
        // 所有「引用的 id 不存在」检查集中在最先：引用断了，后面的结构检查没有报告价值
        // 标题索引只含名字不含事件体，引用解析与查重都单独对它做
        let index = build_event_index(&raw.event);
        check_references(&raw, &index).map_err(|errors| DocumentError::References { errors })?;
        check_acyclic(&raw.entity)?;
        check_unique_event_titles(&index)?;
        let narrative = resolve_narratives(raw.narrative, &index);
        let event = build_events(raw.event)?;
        Ok(WeftDoc {
            story: raw.story,
            entity: raw.entity,
            entity_link: raw.entity_link,
            event,
            narrative,
            calendar: raw.calendar,
            calculable_tag: raw.calculable_tag,
            calendar_meta: None,
        })
    }
}

// 事件 id = `组名/标题`；时间解析要等历法加载，document.rs 里做
fn build_events(
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

#[derive(Debug, Serialize, Deserialize)]
pub struct Story {
    pub title: String,
    pub description: Option<String>,
    pub author: Option<String>,
    // 历法名：内置 gregorian，或顶层 calendar 的注册名；缺省 gregorian
    pub date_mode: Option<String>,
}

// 抽象实体：人物、团体、地点、道具……都算
#[derive(Debug, Serialize, Deserialize)]
pub struct Entity {
    pub name: Option<String>,
    // 时间分量列表（按历法的分量顺序，可省略末尾分量），解析时按历法归一化；
    // 单个整数（如 base_time: 1881）等价于单分量列表
    #[serde(default, deserialize_with = "int_or_seq")]
    pub base_time: Option<Vec<i64>>,
    // humanize 结果，document.rs 加载历法后填充，不来自 YAML
    #[serde(skip_deserializing, skip_serializing_if = "Option::is_none")]
    pub base_time_display: Option<String>,
    pub description: Option<String>,
    // 归属的上层实体 id 列表（如人物 → 团体）；可计算标签算出的归属也并到这里，不单独存
    #[serde(default)]
    pub belong_to: Vec<String>,
    // 可计算标签列表：document.rs 按 base_time 执行后把归属链写进 belong_to，不来自展示数据
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub calculable_tags: Vec<String>,
}

// 实体关系对：entities 恰好两个（YAML 列表反序列化成元组，天然限定一对），relations 是关系名
#[derive(Debug, Serialize, Deserialize)]
pub struct EntityLink {
    pub entities: (String, String),
    pub relations: String,
}

// 顶层 calendar / calculable_tag 共用的插件清单：rhai 脚本 + 固定入口约定
#[derive(Debug, Serialize, Deserialize)]
pub struct CalendarSpec {
    pub runtime: String,
    pub kind: String,
    pub api: i64,
    // 脚本路径，相对故事文件所在目录
    pub source: String,
}

impl CalendarSpec {
    pub fn validate(&self, name: &str, kind: &str) -> Result<(), ValidationError> {
        if self.runtime != "rhai" || self.kind != kind || self.api != 1 {
            return Err(ValidationError::InvalidPluginSpec {
                name: name.to_string(),
                kind: kind.to_string(),
            });
        }
        Ok(())
    }
}

// 相对时间：末尾嵌套列表是锚点，前面的整数是逐分量偏移，可多层嵌套。
// [0, 0, 0, 2, [2025, 4, 2, 10]] = 2025-4-2 10 时加 2 小时
#[derive(Debug, Clone, Serialize)]
pub struct Phase {
    pub components: Vec<i64>,
    pub anchor: Option<Box<Phase>>,
}

impl Phase {
    pub fn from_yaml(value: &Value) -> Result<Self, DocumentError> {
        let sequence = value
            .as_sequence()
            .ok_or_else(|| DocumentError::validation(ValidationError::TimeNonemptyList))?;
        if sequence.is_empty() {
            return Err(ValidationError::TimeNonemptyList.into());
        }
        let (components, anchor) = match sequence.last() {
            Some(Value::Sequence(_)) => (&sequence[..sequence.len() - 1], sequence.last()),
            _ => (&sequence[..], None),
        };
        let mut base = Vec::with_capacity(components.len());
        for component in components {
            base.push(
                component.as_i64().ok_or_else(|| {
                    DocumentError::validation(ValidationError::TimeIntegerComponent)
                })?,
            );
        }
        let anchor = anchor.map(Self::from_yaml).transpose()?.map(Box::new);
        Ok(Self {
            components: base,
            anchor,
        })
    }

    // 逐层分量求和，得到绝对时间
    pub fn flatten(&self) -> Vec<i64> {
        let mut result = Vec::new();
        let mut phase = Some(self);
        while let Some(current) = phase {
            if current.components.len() > result.len() {
                result.resize(current.components.len(), 0);
            }
            for (target, value) in result.iter_mut().zip(&current.components) {
                *target += value;
            }
            phase = current.anchor.as_deref();
        }
        result
    }
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
    // 每个参与实体相对其 base_time 的偏移（document.rs 填充）
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub offsets: Vec<EntityOffset>,
    // 以下由 document.rs 加载历法后填充
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

// YAML 里的事件原始形态，解析成 EventItem 要等历法加载
#[derive(Debug, Deserialize)]
struct RawEvent {
    start_time: Value,
    end_time: Option<Value>,
    #[serde(default)]
    entities: Vec<String>,
    description: Option<String>,
}

// 叙事线：subject 事件按 observer 的 base_time 展开成相对时间轴（与 event 的绝对时间轴相反）
#[derive(Debug, Serialize)]
pub struct Narrative {
    // 观察者，原样保留 YAML 引用（实体 id）
    pub observer: String,
    // 按 document.rs 填充的相对开始时间排序
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
struct RawNarrative {
    subject: Vec<String>,
    observer: String,
}

// 事件标题索引：标题 → 组名列表（组名已排序，查重报告顺序稳定）。
// 裸标题引用靠它定位组名，`组名/标题` 全 id 引用靠它验证存在；
// 不含事件体，单条引用拿索引即可单独解析
type EventIndex<'a> = BTreeMap<&'a str, Vec<&'a str>>;

fn build_event_index(events: &HashMap<String, HashMap<String, RawEvent>>) -> EventIndex<'_> {
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
fn resolve_event<'a>(index: &'a EventIndex<'a>, r: &'a str) -> Option<(&'a str, &'a str)> {
    match r.split_once('/') {
        Some((group, title)) => index
            .get(title)
            .is_some_and(|groups| groups.contains(&group))
            .then_some((group, title)),
        None => index.get(r).map(|groups| (groups[0], r)),
    }
}

// narrative：引用已在 check_references 校验，这里把 subject 重写成事件 id。
// 只依赖标题索引，单条 narrative 给索引即可单独解析
fn resolve_narratives(
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

// 所有「引用的 id 不存在」检查集中在此：belong_to、entity_link、event 参与实体、
// narrative 的 observer 与 subject。先于结构检查（成环、标题重复）报告全部断开的引用
fn check_references(raw: &RawWeftDoc, index: &EventIndex<'_>) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();
    for (id, entity) in &raw.entity {
        for target in &entity.belong_to {
            if !raw.entity.contains_key(target) {
                errors.push(ValidationError::EntityUnknownParent {
                    entity: id.clone(),
                    target: target.clone(),
                });
            }
        }
    }
    for (group, links) in &raw.entity_link {
        for link in links {
            for r in [&link.entities.0, &link.entities.1] {
                if !raw.entity.contains_key(r) {
                    errors.push(ValidationError::UnknownLinkEntity {
                        group: group.clone(),
                        entity: r.clone(),
                    });
                }
            }
        }
    }
    for (group, events) in &raw.event {
        for (title, ev) in events {
            for r in &ev.entities {
                if !raw.entity.contains_key(r) {
                    errors.push(ValidationError::UnknownEventEntity {
                        event: format!("{group}/{title}"),
                        entity: r.clone(),
                    });
                }
            }
        }
    }
    for (name, nar) in &raw.narrative {
        if !raw.entity.contains_key(&nar.observer) {
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
        Err(errors)
    }
}

// 归属图不许成环（如 a belong_to b、b belong_to a），否则按层级递归展示无法终止。
// 可计算标签算出的归属也要复查，所以开放给 document.rs。DFS 三色标记：0 未访问 / 1 在当前路径上 / 2 已完成。
// 状态键借用图里的 id，递归时不再反复克隆
pub(crate) fn check_acyclic(graph: &HashMap<String, Entity>) -> Result<(), DocumentError> {
    fn visit<'a>(
        id: &'a str,
        graph: &'a HashMap<String, Entity>,
        state: &mut HashMap<&'a str, u8>,
    ) -> Result<(), DocumentError> {
        match state.get(id) {
            Some(1) => {
                return Err(ValidationError::EntityCycle {
                    entity: id.to_string(),
                }
                .into());
            }
            Some(2) => return Ok(()),
            _ => {}
        }
        // belong_to 的目标由 check_references / apply_chain 保证存在
        let (key, entity) = graph.get_key_value(id).expect("belong_to 的目标已校验存在");
        let key = key.as_str();
        state.insert(key, 1);
        for parent in &entity.belong_to {
            visit(parent, graph, state)?;
        }
        state.insert(key, 2);
        Ok(())
    }
    let mut state = HashMap::new();
    for id in graph.keys() {
        visit(id, graph, &mut state)?;
    }
    Ok(())
}

// 事件标题全局唯一：跨组同名也算重复，subject 等引用才能省略组名。
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

// base_time 写单个整数（只有年份的场合）等价于单分量列表；空值即无时间
fn int_or_seq<'de, D>(d: D) -> Result<Option<Vec<i64>>, D::Error>
where
    D: Deserializer<'de>,
{
    struct IntOrSeq;

    impl<'de> Visitor<'de> for IntOrSeq {
        type Value = Option<Vec<i64>>;

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("an integer or a sequence of integers")
        }

        fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
            Ok(Some(vec![v]))
        }

        fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
            i64::try_from(v)
                .map(|v| Some(vec![v]))
                .map_err(|_| E::custom("time component does not fit in i64"))
        }

        fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut out = Vec::with_capacity(seq.size_hint().unwrap_or(0));
            while let Some(v) = seq.next_element::<i64>()? {
                out.push(v);
            }
            Ok(Some(out))
        }
    }

    d.deserialize_any(IntOrSeq)
}
