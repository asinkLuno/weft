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
}

// 抽象实体：人物、团体、地点、道具……都算
#[derive(Debug, Serialize, Deserialize)]
pub struct Entity {
    pub name: Option<String>,
    // shortcut: 类型未定，先不校验；确定后收紧
    pub base_time: Option<Value>,
    pub description: Option<String>,
    // 归属的上层实体 id 列表（如人物 → 团体）
    #[serde(default)]
    pub belong_to: Vec<String>,
}
