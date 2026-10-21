use std::collections::HashMap;

use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_yaml_ng::Value;

// YAML 数据模型：顶层键逐个补齐；反序列化即校验+滤掉未声明键，再序列化成 JSON 给前端
#[derive(Debug, Serialize, Deserialize)]
pub struct StoryDoc {
    pub story: Story,
    // YAML 对重复键是后者覆盖，id 唯一性必须自己查
    #[serde(default, deserialize_with = "unique_entity")]
    pub entity: HashMap<String, Entity>,
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
                if let Some(target) = &entity.belong_to {
                    if !out.contains_key(target) {
                        return Err(de::Error::custom(format!(
                            "entity `{id}` belong_to unknown entity `{target}`"
                        )));
                    }
                }
            }
            Ok(out)
        }
    }

    d.deserialize_map(EntityMap)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Story {
    pub title: String,
    pub description: Option<String>,
    pub author: Option<String>,
}

// 抽象实体：人物、团体、地点、道具……都算
#[derive(Debug, Serialize, Deserialize)]
pub struct Entity {
    pub name: Option<String>,
    // shortcut: 类型未定，先不校验；确定后收紧
    pub base_time: Option<Value>,
    pub description: Option<String>,
    // 归属的上层实体 id（如人物 → 团体）
    pub belong_to: Option<String>,
}
