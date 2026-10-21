use std::collections::{BTreeMap, HashMap, HashSet};

use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_yaml_ng::Value;

use crate::calendar::CalendarMetadata;

// 实体关系对：entities 恰好两个（YAML 列表反序列化成元组，天然限定一对），relations 是关系名
#[derive(Debug, Serialize, Deserialize)]
pub struct EntityLink {
    pub entities: (String, String),
    pub relations: String,
}

// YAML 数据模型：顶层键逐个补齐；反序列化即校验+滤掉未声明键，再序列化成 JSON 给前端。
// entity_link 的引用校验要看到 entity 表（跨字段），所以 StoryDoc 手写 Deserialize、内嵌 Raw 承接字段
#[derive(Debug, Serialize)]
pub struct StoryDoc {
    pub story: Story,
    // YAML 对重复键是后者覆盖，id 唯一性必须自己查
    #[serde(default, deserialize_with = "unique_entity")]
    pub entity: HashMap<String, Entity>,
    // 关系分组：组名 → 关系对列表
    #[serde(default)]
    pub entity_link: HashMap<String, Vec<EntityLink>>,
    // 自定义历法注册表：注册名 → Rhai 脚本清单（相对故事文件的路径）
    #[serde(default)]
    pub calendar: HashMap<String, CalendarSpec>,
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

impl<'de> Deserialize<'de> for StoryDoc {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            story: Story,
            #[serde(default, deserialize_with = "unique_entity")]
            entity: HashMap<String, Entity>,
            #[serde(default)]
            entity_link: HashMap<String, Vec<EntityLink>>,
            #[serde(default)]
            event: HashMap<String, HashMap<String, RawEvent>>,
            #[serde(default)]
            narrative: HashMap<String, RawNarrative>,
            #[serde(default)]
            calendar: HashMap<String, CalendarSpec>,
        }
        let raw = Raw::deserialize(d)?;
        // 事件标题全局唯一：跨组同名也算重复，subject 等引用才能省略组名
        let mut seen = HashSet::new();
        for events in raw.event.values() {
            for title in events.keys() {
                if !seen.insert(title.as_str()) {
                    return Err(de::Error::custom(format!("事件标题 `{title}` 重复")));
                }
            }
        }
        // 引用按 id 或名称对回实体（与 belong_to 的 id 校验同理，名称匹配放宽给手写 YAML）
        for (group, links) in &raw.entity_link {
            for link in links {
                for r in [&link.entities.0, &link.entities.1] {
                    if !entity_exists(&raw.entity, r) {
                        return Err(de::Error::custom(format!(
                            "entity_link `{group}` 引用了不存在的实体 `{r}`"
                        )));
                    }
                }
            }
        }
        // 事件的实体引用按 id 或名称对回实体表（与 entity_link 同规则）
        for (group, events) in &raw.event {
            for (title, ev) in events {
                for r in &ev.entities {
                    if !entity_exists(&raw.entity, r) {
                        return Err(de::Error::custom(format!(
                            "event `{group}/{title}` 引用了不存在的实体 `{r}`"
                        )));
                    }
                }
            }
        }
        // narrative：observer 对回实体、subject 对回事件，校验时顺带重写成事件 id
        let mut narrative = BTreeMap::new();
        for (name, nar) in raw.narrative {
            if !entity_exists(&raw.entity, &nar.observer) {
                return Err(de::Error::custom(format!(
                    "narrative `{name}` 引用了不存在的实体 `{}`",
                    nar.observer
                )));
            }
            let mut subject = Vec::with_capacity(nar.subject.len());
            for r in nar.subject {
                let Some((g, t)) = resolve_event(&raw.event, &r) else {
                    return Err(de::Error::custom(format!(
                        "narrative `{name}` 的 subject 引用了不存在的事件 `{r}`"
                    )));
                };
                subject.push(NarrativeSubject {
                    event: format!("{g}/{t}"),
                    start_tick: None,
                    end_tick: None,
                    start: None,
                    end: None,
                });
            }
            narrative.insert(name, Narrative { observer: nar.observer, subject });
        }
        // 事件 id = `组名/标题`；时间解析要等历法加载，document.rs 里做
        let mut event: BTreeMap<String, Vec<EventItem>> = BTreeMap::new();
        for (group, events) in raw.event {
            let mut list = Vec::with_capacity(events.len());
            for (title, ev) in events {
                list.push(EventItem {
                    id: format!("{group}/{title}"),
                    title,
                    description: ev.description,
                    entities: ev.entities,
                    offsets: Vec::new(),
                    start_phase: ev.start_time,
                    end_phase: ev.end_time,
                    start: None,
                    end: None,
                    start_tick: None,
                    end_tick: None,
                    start_display: None,
                    end_display: None,
                });
            }
            event.insert(group, list);
        }
        Ok(StoryDoc {
            story: raw.story,
            entity: raw.entity,
            entity_link: raw.entity_link,
            event,
            narrative,
            calendar: raw.calendar,
            calendar_meta: None,
        })
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
    pub fn from_yaml(value: &Value) -> Result<Self, String> {
        let sequence: Vec<Value> = serde_yaml_ng::from_value(value.clone())
            .map_err(|_| "时间必须是非空列表".to_string())?;
        if sequence.is_empty() {
            return Err("时间必须是非空列表".into());
        }
        let (components, anchor) = match sequence.last() {
            Some(Value::Sequence(_)) => (&sequence[..sequence.len() - 1], sequence.last()),
            _ => (&sequence[..], None),
        };
        let mut base = Vec::with_capacity(components.len());
        for component in components {
            base.push(component.as_i64().ok_or("时间分量必须是整数")?);
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

fn phase_from_yaml<'de, D>(d: D) -> Result<Phase, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(d)?;
    Phase::from_yaml(&value).map_err(de::Error::custom)
}

// end_time 显式写 null 等同缺省
fn optional_phase<'de, D>(d: D) -> Result<Option<Phase>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(d)?;
    if value.is_null() {
        return Ok(None);
    }
    Phase::from_yaml(&value).map(Some).map_err(de::Error::custom)
}

// YAML 里的事件原始形态，解析成 EventItem 要等历法加载
#[derive(Debug, Deserialize)]
struct RawEvent {
    #[serde(deserialize_with = "phase_from_yaml")]
    start_time: Phase,
    #[serde(default, deserialize_with = "optional_phase")]
    end_time: Option<Phase>,
    #[serde(default)]
    entities: Vec<String>,
    description: Option<String>,
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

// 事件（旧版 drift）：按组组织，id = `组名/标题`
#[derive(Debug, Serialize)]
pub struct EventItem {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    // 参与实体，原样保留 YAML 引用（id 或名称）
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entities: Vec<String>,
    // 每个参与实体相对其 base_time 的偏移（document.rs 填充，旧版 journal）
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

// 叙事线：subject 事件按 observer 的 base_time 展开成相对时间轴（与 event 的绝对时间轴相反）
#[derive(Debug, Serialize)]
pub struct Narrative {
    // 观察者，原样保留 YAML 引用（id 或名称）
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

// 引用按 id 或名称对回实体（entity_link / event / narrative 同规则）
fn entity_exists(entity: &HashMap<String, Entity>, r: &str) -> bool {
    entity.contains_key(r)
        || entity.values().any(|e| e.name.as_deref() == Some(r))
}

// 事件引用：`组名/标题` 全 id，或裸标题（标题已全局唯一校验）
fn resolve_event(
    event: &HashMap<String, HashMap<String, RawEvent>>,
    r: &str,
) -> Option<(String, String)> {
    if let Some((g, t)) = r.split_once('/') {
        return event
            .get(g)
            .is_some_and(|evs| evs.contains_key(t))
            .then(|| (g.to_string(), t.to_string()));
    }
    event
        .iter()
        .find(|(_, evs)| evs.contains_key(r))
        .map(|(g, _)| (g.clone(), r.to_string()))
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
            Ok(Some(vec![v as i64]))
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

// 顶层 calendar 的注册清单，格式沿旧版：rhai 脚本 + 固定入口约定
#[derive(Debug, Serialize, Deserialize)]
pub struct CalendarSpec {
    pub runtime: String,
    pub kind: String,
    pub api: i64,
    // 脚本路径，相对故事文件所在目录
    pub source: String,
}

impl CalendarSpec {
    pub fn validate(&self, name: &str) -> Result<(), String> {
        if self.runtime != "rhai" || self.kind != "calendar" || self.api != 1 {
            return Err(format!(
                "历法 `{name}` 需要 runtime: rhai, kind: calendar, api: 1"
            ));
        }
        Ok(())
    }
}

fn unique_entity<'de, D>(d: D) -> Result<HashMap<String, Entity>, D::Error>
where
    D: Deserializer<'de>,
{
    struct EntityMap;

    impl<'de> Visitor<'de> for EntityMap {
        type Value = HashMap<String, Entity>;

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a map of entity id to entity")
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut out = HashMap::with_capacity(map.size_hint().unwrap_or(0));
            while let Some((id, entity)) = map.next_entry::<String, Entity>()? {
                if out.insert(id.clone(), entity).is_some() {
                    return Err(de::Error::custom(format!("duplicate entity id `{id}`")));
                }
            }
            // 引用完整性：belong_to 必须指向表里已有的 entity
            for (id, entity) in &out {
                for target in &entity.belong_to {
                    if !out.contains_key(target) {
                        return Err(de::Error::custom(format!(
                            "entity `{id}` belong_to unknown entity `{target}`"
                        )));
                    }
                }
            }
            check_acyclic(&out).map_err(de::Error::custom)?;
            Ok(out)
        }
    }

    d.deserialize_map(EntityMap)
}

// 归属图不许成环（如 a belong_to b、b belong_to a），否则按层级递归展示无法终止。
// DFS 三色标记：0 未访问 / 1 在当前路径上 / 2 已完成
fn check_acyclic(graph: &HashMap<String, Entity>) -> Result<(), String> {
    fn visit(
        id: &str,
        graph: &HashMap<String, Entity>,
        state: &mut HashMap<String, u8>,
    ) -> Result<(), String> {
        match state.get(id) {
            Some(1) => return Err(format!("entity `{id}` belong_to forms a cycle")),
            Some(2) => return Ok(()),
            _ => {}
        }
        state.insert(id.to_string(), 1);
        for parent in &graph[id].belong_to {
            visit(parent, graph, state)?;
        }
        state.insert(id.to_string(), 2);
        Ok(())
    }
    let mut state = HashMap::new();
    for id in graph.keys() {
        visit(id, graph, &mut state)?;
    }
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Story {
    pub title: String,
    pub description: Option<String>,
    pub author: Option<String>,
    // 历法名：内置 gregorian / gregorian_en，或顶层 calendar 的注册名；缺省 gregorian
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
    // 归属的上层实体 id 列表（如人物 → 团体）
    #[serde(default)]
    pub belong_to: Vec<String>,
}
