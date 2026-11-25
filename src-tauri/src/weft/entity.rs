//! 实体、归属图与关系模型：编译期输入结构、构建上下文与运行时结构，
//! 以及校验、可计算属性执行和图构建的完整流程。

use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::Path,
};

use daggy::{Dag, Walker};
use serde::Serialize;
use serde_json::{Value as JsonValue, json};

use crate::calculable_prop::CalculablePropRuntime;
use crate::calendar::{Calendar, Moment};
use crate::weft::errors::{DocumentError, SourceSpan, ValidationError};
use crate::weft::phase::Phase;

// ---- 输入结构 ----

#[derive(Debug)]
pub(crate) struct RawEntity {
    pub base_time: Option<(Phase, SourceSpan)>,
    pub description: Option<String>,
    pub avatar: Option<(String, SourceSpan)>,
    pub belong_to: Vec<(String, SourceSpan)>,
    pub calculable_props: Vec<(String, SourceSpan)>,
    pub extra_props: HashMap<String, JsonValue>,
}

#[derive(Debug)]
pub(crate) struct RawEntityLink {
    pub left: (String, SourceSpan),
    pub right: (String, SourceSpan),
    pub relations: String,
}

// ---- 运行时结构 ----

/// 已完成头像解析和基准时间展示归一化的实体；事件通过实体 ID 引用。
#[derive(Debug, Serialize)]
pub struct Entity {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    pub base_time: Option<Phase>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_time_humanizer: Option<String>,
    pub description: Option<String>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub extra_props: HashMap<String, JsonValue>,
}

/// 归属 DAG 节点：只含实体 ID 与构建期反查出的直接上级 ID；无上级时省略 belong_to。
#[derive(Debug, Serialize)]
pub struct BelongNode {
    pub id: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub belong_to: Vec<String>,
}

/// 一条关系：两个实体 ID 与关系类型；ID 是否合法由构建管线统一校验。
#[derive(Debug, Serialize)]
pub struct EntityLink {
    pub entities: (String, String),
    pub relations: String,
}

// ---- 构建管线 ----

pub(crate) struct EntityBuildContext<'a> {
    pub base: &'a Path,
    pub calendar: &'a Calendar,
    pub calculable_props: &'a CalculablePropRuntime,
}

/// 构建实体段：校验输入，RawEntity 转成 Entity 并执行可计算属性，
/// 结果写进 extra_props。归属 DAG 的边已去重且无环，反查出的直接上级写进 belong_dag。
pub(crate) fn build_entities(
    raw_entities: BTreeMap<String, RawEntity>,
    raw_links: &BTreeMap<String, Vec<RawEntityLink>>,
    context: EntityBuildContext<'_>,
) -> Result<(HashMap<String, Entity>, Vec<BelongNode>, HashMap<String, Vec<EntityLink>>), DocumentError>
{
    let belong_graph = build_belong_graph(&raw_entities, raw_links)?;
    validate_avatars(&raw_entities, context.base)?;
    let parents_by_id: HashMap<&str, Vec<String>> = belong_graph
        .graph()
        .node_indices()
        .map(|node| {
            let mut parents: Vec<_> = belong_graph
                .parents(node)
                .iter(&belong_graph)
                .map(|(_, parent)| belong_graph[parent].clone())
                .collect();
            parents.sort_unstable();
            (belong_graph[node].as_str(), parents)
        })
        .collect();

    let mut entities = HashMap::with_capacity(raw_entities.len());
    for (id, input) in raw_entities {
        let belong_to = parents_by_id.get(id.as_str()).cloned().unwrap_or_default();
        let base_time_moment = input
            .base_time
            .as_ref()
            .map(|(phase, span)| {
                context
                    .calendar
                    .resolve(phase)
                    .map_err(|err| err.with_span(span.clone()))
            })
            .transpose()?;
        let base_time_humanizer = base_time_moment
            .as_ref()
            .map(|moment| context.calendar.humanize(moment))
            .transpose()?;
        let avatar = input
            .avatar
            .map(|(source, _)| source)
            .filter(|source| !source.trim().is_empty())
            .and_then(|source| fs::canonicalize(context.base.join(source)).ok())
            .filter(|path| path.is_file())
            .map(|path| path.to_string_lossy().into_owned());
        let mut entity = Entity {
            id: id.clone(),
            avatar,
            base_time: input.base_time.map(|(phase, _)| phase),
            base_time_humanizer,
            description: input.description,
            extra_props: input.extra_props,
        };
        apply_calculable_props(
            &mut entity,
            &input.calculable_props,
            &belong_to,
            base_time_moment.as_ref(),
            &context,
        )?;
        entities.insert(id, entity);
    }

    let mut belong_dag: Vec<_> = parents_by_id
        .into_iter()
        .map(|(id, belong_to)| BelongNode {
            id: id.to_owned(),
            belong_to,
        })
        .collect();
    belong_dag.sort_unstable_by(|a, b| a.id.cmp(&b.id));

    let entity_link = raw_links
        .iter()
        .map(|(group, edges)| {
            (
                group.clone(),
                edges
                    .iter()
                    .map(|edge| EntityLink {
                        entities: (edge.left.0.clone(), edge.right.0.clone()),
                        relations: edge.relations.clone(),
                    })
                    .collect(),
            )
        })
        .collect();
    Ok((entities, belong_dag, entity_link))
}

fn build_belong_graph(
    entities: &BTreeMap<String, RawEntity>,
    links: &BTreeMap<String, Vec<RawEntityLink>>,
) -> Result<Dag<String, ()>, DocumentError> {
    for (id, raw) in entities {
        for (parent, span) in &raw.belong_to {
            if !entities.contains_key(parent) {
                return Err(DocumentError::validation_at(
                    ValidationError::EntityUnknownParent {
                        entity: id.clone(),
                        target: parent.clone(),
                    },
                    span.clone(),
                ));
            }
        }
    }
    for (group, edges) in links {
        for link in edges {
            for (id, span) in [&link.left, &link.right] {
                if !entities.contains_key(id) {
                    return Err(DocumentError::validation_at(
                        ValidationError::UnknownLinkEntity {
                            group: group.clone(),
                            entity: id.clone(),
                        },
                        span.clone(),
                    ));
                }
            }
        }
    }
    let mut graph = Dag::new();
    let indices: HashMap<_, _> = entities
        .keys()
        .map(|id| (id.as_str(), graph.add_node(id.clone())))
        .collect();
    for (id, raw) in entities {
        for (parent, span) in &raw.belong_to {
            graph
                .update_edge(indices[parent.as_str()], indices[id.as_str()], ())
                .map_err(|_| {
                    DocumentError::validation_at(
                        ValidationError::EntityCycle { entity: id.clone() },
                        span.clone(),
                    )
                })?;
        }
    }
    Ok(graph)
}

fn validate_avatars(
    entities: &BTreeMap<String, RawEntity>,
    base: &Path,
) -> Result<(), DocumentError> {
    for (id, input) in entities {
        let Some((source, span)) = input
            .avatar
            .as_ref()
            .filter(|(source, _)| !source.trim().is_empty())
        else {
            continue;
        };
        let valid = fs::canonicalize(base.join(source))
            .map(|path| path.is_file() && is_image(&path))
            .unwrap_or(false);
        if !valid {
            return Err(DocumentError::validation_at(
                ValidationError::InvalidAvatarPath {
                    entity: id.clone(),
                    path: source.clone(),
                },
                span.clone(),
            ));
        }
    }
    Ok(())
}

fn apply_calculable_props(
    entity: &mut Entity,
    props: &[(String, SourceSpan)],
    belong_to: &[String],
    base_time_moment: Option<&Moment>,
    context: &EntityBuildContext<'_>,
) -> Result<(), DocumentError> {
    if props.is_empty() {
        return Ok(());
    }
    let base_time = base_time_moment.map(|moment| {
        json!({
            "values": moment.values,
            "precision": moment.precision,
            "tick": moment.tick,
        })
    });
    let raw_extra_props = entity.extra_props.clone();
    for (name, span) in props {
        let input = json!({
            "id": entity.id,
            "base_time": base_time,
            "description": entity.description,
            "belong_to": belong_to,
            "extra_props": raw_extra_props,
        });
        let value = context
            .calculable_props
            .calculable_prop(name, input)
            .map_err(|err| err.with_span(span.clone()))?;
        if let Some(value) = value {
            entity.extra_props.insert(name.clone(), value);
        }
    }
    Ok(())
}

// ---- 校验工具 ----

const IMAGE_EXTENSIONS: [&str; 8] = ["bmp", "gif", "ico", "jpeg", "jpg", "png", "svg", "webp"];

fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            IMAGE_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
        })
}
