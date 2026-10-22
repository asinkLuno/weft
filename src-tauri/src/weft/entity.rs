use std::collections::HashMap;

use serde::de;
use serde::{Deserialize, Deserializer, Serialize};

use crate::calendar::Calendar;
use crate::weft::errors::{DocumentError, ValidationError};
use crate::weft::phase::Phase;

// 抽象实体：人物、团体、地点、道具……都算
#[derive(Debug, Serialize, Deserialize)]
pub struct Entity {
    pub name: Option<String>,
    // 原始 Phase 用于偏移运算，展示归一化不会改写它。
    #[serde(
        default,
        rename = "base_time",
        skip_serializing,
        deserialize_with = "deserialize_base_phase"
    )]
    pub base_phase: Option<Phase>,
    #[serde(skip_deserializing)]
    pub base_time: Option<Vec<i64>>,
    // humanize 结果，加载历法后由 apply_times 填充，不来自 YAML
    #[serde(skip_deserializing, skip_serializing_if = "Option::is_none")]
    pub base_time_display: Option<String>,
    pub description: Option<String>,
    // 归属的上层实体 id 列表（如人物 → 团体）；可计算标签算出的归属也并到这里，不单独存
    #[serde(default)]
    pub belong_to: Vec<String>,
    // 可计算标签列表：weft_doc.rs 按 base_time 执行后把归属链写进 belong_to，不来自展示数据
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub calculable_tags: Vec<String>,
}

// 实体关系对：entities 恰好两个（YAML 列表反序列化成元组，天然限定一对），relations 是关系名
#[derive(Debug, Serialize, Deserialize)]
pub struct EntityLink {
    pub entities: (String, String),
    pub relations: String,
}

// entity 段检查：先引用（belong_to 与 entity_link 的目标存在）后结构（归属图无环）。
// 引用错误批量收集，一次报出该段全部断链
pub(crate) fn check(
    entity: &HashMap<String, Entity>,
    links: &HashMap<String, Vec<EntityLink>>,
) -> Result<(), DocumentError> {
    let mut errors = Vec::new();
    for (id, e) in entity {
        for target in &e.belong_to {
            if !entity.contains_key(target) {
                errors.push(ValidationError::EntityUnknownParent {
                    entity: id.clone(),
                    target: target.clone(),
                });
            }
        }
    }
    for (group, pairs) in links {
        for link in pairs {
            for r in [&link.entities.0, &link.entities.1] {
                if !entity.contains_key(r) {
                    errors.push(ValidationError::UnknownLinkEntity {
                        group: group.clone(),
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
    check_acyclic(entity)
}

// 归属图不许成环（如 a belong_to b、b belong_to a），否则按层级递归展示无法终止。
// 可计算标签算出的归属也要复查，所以开放给 weft_doc.rs。DFS 三色标记：0 未访问 / 1 在当前路径上 / 2 已完成。
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
        // belong_to 的目标由段内引用检查 / apply_chain 保证存在
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

// 单个整数仍等价于单分量 Phase；列表支持递归嵌套。
fn deserialize_base_phase<'de, D>(d: D) -> Result<Option<Phase>, D::Error>
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

// 展示值单独保存；原始 Phase 留给事件做减法。
pub(crate) fn apply_times(
    entity: &mut HashMap<String, Entity>,
    cal: &Calendar,
) -> Result<(), DocumentError> {
    for entity in entity.values_mut() {
        if let Some(phase) = &entity.base_phase {
            let normalized = cal.normalize(&phase.flatten(cal.component_count())?)?;
            entity.base_time_display = Some(cal.humanize(&normalized)?);
            entity.base_time = Some(normalized);
        }
    }
    Ok(())
}

// 按实体 id 取 base_time；事件与叙事线都要拿它算偏移
pub(crate) fn resolve_base<'a>(entity: &'a HashMap<String, Entity>, id: &str) -> Option<&'a Phase> {
    entity.get(id).and_then(|e| e.base_phase.as_ref())
}
