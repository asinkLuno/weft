use std::{collections::HashMap, sync::Arc};

use serde::de;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::calculable_tag::CalculableTagRuntime;
use crate::calendar::Calendar;
use crate::weft::errors::{DocumentError, ValidationError};
use crate::weft::phase::Phase;

// 集合与归属边共享同一份实体；归属图经过无环校验，不会形成 Arc 循环。
#[derive(Debug, Serialize)]
pub struct Entity {
    pub id: String,
    // 原始 Phase 用于偏移运算，展示归一化不会改写它。
    pub base_time: Option<Phase>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_time_humanizer: Option<String>,
    pub description: Option<String>,
    #[serde(serialize_with = "serialize_parents")]
    pub belong_to: Vec<Arc<Entity>>,
}

fn serialize_parents<S: Serializer>(
    parents: &[Arc<Entity>],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    parents
        .iter()
        .map(|parent| &parent.id)
        .collect::<Vec<_>>()
        .serialize(serializer)
}

// 可计算标签仅用于输入阶段，执行结果在构建 Entity 时成为 belong_to 引用。
#[derive(Debug, Default, Deserialize)]
pub(crate) struct RawEntity {
    #[serde(default, deserialize_with = "deserialize_base_time")]
    pub base_time: Option<Phase>,
    pub description: Option<String>,
    #[serde(default)]
    pub belong_to: Vec<String>,
    #[serde(default)]
    pub calculable_tags: Vec<String>,
}

impl Entity {
    pub(crate) fn new(
        id: String,
        raw: RawEntity,
        belong_to: Vec<Arc<Entity>>,
        cal: &Calendar,
    ) -> Result<Self, DocumentError> {
        let base_time_humanizer = raw
            .base_time
            .as_ref()
            .map(|phase| {
                let normalized = cal.normalize(&phase.flatten(cal.component_count())?)?;
                cal.humanize(&normalized)
            })
            .transpose()?;
        Ok(Self {
            id,
            base_time: raw.base_time,
            base_time_humanizer,
            description: raw.description,
            belong_to,
        })
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawEntityLink {
    pub entities: (String, String),
    pub relations: String,
}

#[derive(Debug, Serialize)]
pub struct EntityLink {
    #[serde(serialize_with = "serialize_endpoints")]
    pub entities: (Arc<Entity>, Arc<Entity>),
    pub relations: String,
}

fn serialize_endpoints<S: Serializer>(
    entities: &(Arc<Entity>, Arc<Entity>),
    serializer: S,
) -> Result<S::Ok, S::Error> {
    (&entities.0.id, &entities.1.id).serialize(serializer)
}

#[derive(Debug)]
pub struct EntityCollection {
    entities: HashMap<String, Arc<Entity>>,
    pub entity_link: HashMap<String, Vec<EntityLink>>,
}

impl Serialize for EntityCollection {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let entities: HashMap<_, _> = self
            .entities
            .iter()
            .map(|(id, entity)| (id, entity.as_ref()))
            .collect();
        let mut state = serializer.serialize_struct("EntityCollection", 2)?;
        state.serialize_field("entity", &entities)?;
        state.serialize_field("entity_link", &self.entity_link)?;
        state.end()
    }
}

impl EntityCollection {
    pub(crate) fn new(
        mut raw: HashMap<String, RawEntity>,
        links: HashMap<String, Vec<RawEntityLink>>,
        cal: &Calendar,
        calculable_tags: &CalculableTagRuntime,
        mode: Option<&str>,
        lang: &str,
    ) -> Result<Self, DocumentError> {
        // 脚本已在 story 阶段加载；计算归属后才构建 Entity。
        check(&raw, &links)?;
        apply_calculable_tags(&mut raw, calculable_tags, mode, lang, cal)?;
        check(&raw, &links)?;
        // 先构建父节点，再构建子节点；缓存确保所有引用指向相同实例。
        fn build(
            id: &str,
            raw: &mut HashMap<String, RawEntity>,
            entities: &mut HashMap<String, Arc<Entity>>,
            cal: &Calendar,
        ) -> Result<Arc<Entity>, DocumentError> {
            if let Some(entity) = entities.get(id) {
                return Ok(Arc::clone(entity));
            }
            let input = raw.remove(id).expect("实体引用已校验存在且归属无环");
            let mut parents = Vec::new();
            for parent in &input.belong_to {
                parents.push(build(parent, raw, entities, cal)?);
            }
            let entity = Arc::new(Entity::new(id.to_owned(), input, parents, cal)?);
            entities.insert(id.to_owned(), Arc::clone(&entity));
            Ok(entity)
        }
        let ids: Vec<_> = raw.keys().cloned().collect();
        let mut entities = HashMap::new();
        for id in ids {
            build(&id, &mut raw, &mut entities, cal)?;
        }
        let entity_link = links
            .into_iter()
            .map(|(group, links)| {
                let links = links
                    .into_iter()
                    .map(|link| EntityLink {
                        entities: (
                            Arc::clone(&entities[&link.entities.0]),
                            Arc::clone(&entities[&link.entities.1]),
                        ),
                        relations: link.relations,
                    })
                    .collect();
                (group, links)
            })
            .collect();
        Ok(Self {
            entities,
            entity_link,
        })
    }

    pub fn as_map(&self) -> &HashMap<String, Arc<Entity>> {
        &self.entities
    }
}

fn check(
    entity: &HashMap<String, RawEntity>,
    links: &HashMap<String, Vec<RawEntityLink>>,
) -> Result<(), DocumentError> {
    let mut errors = Vec::new();
    for (id, input) in entity {
        for parent in &input.belong_to {
            if !entity.contains_key(parent) {
                errors.push(ValidationError::EntityUnknownParent {
                    entity: id.clone(),
                    target: parent.clone(),
                });
            }
        }
    }
    for (group, pairs) in links {
        for link in pairs {
            for id in [&link.entities.0, &link.entities.1] {
                if !entity.contains_key(id) {
                    errors.push(ValidationError::UnknownLinkEntity {
                        group: group.clone(),
                        entity: id.clone(),
                    });
                }
            }
        }
    }
    if !errors.is_empty() {
        errors.sort_by_cached_key(ToString::to_string);
        return Err(DocumentError::References { errors });
    }
    fn visit<'a>(
        id: &'a str,
        graph: &'a HashMap<String, RawEntity>,
        state: &mut HashMap<&'a str, u8>,
    ) -> Result<(), DocumentError> {
        match state.get(id) {
            Some(1) => {
                return Err(ValidationError::EntityCycle {
                    entity: id.to_owned(),
                }
                .into());
            }
            Some(2) => return Ok(()),
            _ => {}
        }
        state.insert(id, 1);
        for parent in &graph[id].belong_to {
            visit(parent, graph, state)?;
        }
        state.insert(id, 2);
        Ok(())
    }
    let mut state = HashMap::new();
    for id in entity.keys() {
        visit(id, entity, &mut state)?;
    }
    Ok(())
}

fn apply_calculable_tags(
    raw: &mut HashMap<String, RawEntity>,
    runtime: &CalculableTagRuntime,
    mode: Option<&str>,
    lang: &str,
    cal: &Calendar,
) -> Result<(), DocumentError> {
    let mut ids: Vec<_> = raw.keys().cloned().collect();
    ids.sort();
    for id in ids {
        for name in std::mem::take(&mut raw.get_mut(&id).expect("输入实体存在").calculable_tags)
        {
            let entity = &raw[&id];
            let base_time = entity
                .base_time
                .as_ref()
                .map(|phase| cal.normalize(&phase.flatten(cal.component_count())?))
                .transpose()?;
            let context = serde_json::json!({
                "api": 1, "date_mode": mode, "language": lang,
                "entity": { "id": id, "base_time": base_time, "description": entity.description, "belong_to": entity.belong_to },
            });
            let chain = runtime.calculable_tag(&name, &context, lang)?;
            apply_chain(raw, &id, &chain);
        }
    }
    Ok(())
}

fn apply_chain(raw: &mut HashMap<String, RawEntity>, id: &str, chain: &[String]) {
    let Some(first) = chain.first() else {
        return;
    };
    let existing: Vec<_> = chain.iter().map(|id| raw.contains_key(id)).collect();
    for (i, target) in chain.iter().enumerate() {
        if existing[i] {
            break;
        }
        raw.insert(
            target.clone(),
            RawEntity {
                belong_to: chain.get(i + 1).cloned().into_iter().collect(),
                ..RawEntity::default()
            },
        );
    }
    let owner = raw.get_mut(id).expect("可计算标签的源实体存在");
    if !owner.belong_to.contains(first) {
        owner.belong_to.push(first.clone());
    }
}

// 单个整数仍等价于单分量 Phase；列表支持递归嵌套。
fn deserialize_base_time<'de, D>(d: D) -> Result<Option<Phase>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_yaml_ng::Value::deserialize(d)?;
    let value = match value {
        serde_yaml_ng::Value::Null => return Ok(None),
        serde_yaml_ng::Value::Number(_) => serde_yaml_ng::Value::Sequence(vec![value]),
        value => value,
    };
    Phase::from_yaml(&value)
        .map(Some)
        .map_err(de::Error::custom)
}

// 按实体 id 取原始 base_time，供事件与叙事线计算偏移。
pub(crate) fn resolve_base<'a>(
    entity: &'a HashMap<String, Arc<Entity>>,
    id: &str,
) -> Option<&'a Phase> {
    entity.get(id).and_then(|entity| entity.base_time.as_ref())
}
