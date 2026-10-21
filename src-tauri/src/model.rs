use std::collections::HashMap;

use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_yaml_ng::Value;

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
        }
        let raw = Raw::deserialize(d)?;
        // 引用按 id 或名称对回实体（与 belong_to 的 id 校验同理，名称匹配放宽给手写 YAML）
        for (group, links) in &raw.entity_link {
            for link in links {
                for r in [&link.entities.0, &link.entities.1] {
                    let ok = raw.entity.contains_key(r)
                        || raw
                            .entity
                            .values()
                            .any(|e| e.name.as_deref() == Some(r.as_str()));
                    if !ok {
                        return Err(de::Error::custom(format!(
                            "entity_link `{group}` 引用了不存在的实体 `{r}`"
                        )));
                    }
                }
            }
        }
        Ok(StoryDoc {
            story: raw.story,
            entity: raw.entity,
            entity_link: raw.entity_link,
        })
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
