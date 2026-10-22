//! 校验输入、执行标签，并构建共享的实体与关系图。

use std::{collections::HashMap, sync::Arc};

use serde::{Serialize, Serializer, ser::SerializeStruct};

use crate::calculable_tag::CalculableTagRuntime;
use crate::calendar::Calendar;
use crate::weft::errors::DocumentError;

use super::tags::apply_calculable_tags;
use super::validation::validate;
use super::{Entity, EntityLink, RawEntity, RawEntityLink};

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
        validate(&raw, &links)?;
        apply_calculable_tags(&mut raw, calculable_tags, mode, lang, cal)?;
        validate(&raw, &links)?;
        let ids: Vec<_> = raw.keys().cloned().collect();
        let mut entities = HashMap::new();
        for id in ids {
            build_entity(&id, &mut raw, &mut entities, cal)?;
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

// 先构建父节点，再构建子节点；缓存确保所有引用指向相同实例。
fn build_entity(
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
        parents.push(build_entity(parent, raw, entities, cal)?);
    }
    let entity = Arc::new(Entity::new(id.to_owned(), input, parents, cal)?);
    entities.insert(id.to_owned(), Arc::clone(&entity));
    Ok(entity)
}
