//! 实体、归属图与关系模型：编译期输入结构、构建上下文与运行时结构，
//! 以及多诊断校验、可计算属性执行和图构建的完整流程。

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::Path,
};

use daggy::{Dag, Walker};
use serde::Serialize;
use serde_json::{Value as JsonValue, json};

use crate::calculable_prop::CalculablePropRuntime;
use crate::calendar::{Calendar, Moment};
use crate::errors::{ByteSpan, Diagnostic, ValidationError};
use crate::lint::suggest_similar;
use crate::phase::Phase;

// ---- 输入结构 ----

#[derive(Debug)]
pub(crate) struct RawEntity {
    pub name_span: ByteSpan,
    pub base_time: Option<(Phase, ByteSpan)>,
    pub description: Option<String>,
    pub avatar: Option<(String, ByteSpan)>,
    pub belong_to: Vec<(String, ByteSpan)>,
    pub tags: Vec<(String, ByteSpan)>,
    pub calculable_props: Vec<(String, ByteSpan)>,
    pub extra_props: HashMap<String, JsonValue>,
}

#[derive(Debug)]
pub(crate) struct RawEntityLink {
    pub left: (String, ByteSpan),
    pub right: (String, ByteSpan),
    pub relations: String,
    pub directed: bool,
    pub span: ByteSpan,
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
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

/// 一条关系：两个实体 ID、关系类型与方向；`directed = true` 时方向为 `entities.0 -> entities.1`。
#[derive(Debug, Serialize)]
pub struct EntityLink {
    pub entities: (String, String),
    pub relations: String,
    pub directed: bool,
}

// ---- 静态校验（不依赖 Rune 脚本） ----

/// 校验归属图引用、关系实体引用、归属环与头像文件，并收集全部错误。
pub(crate) fn validate_entities_static(
    entities: &BTreeMap<String, RawEntity>,
    links: &BTreeMap<String, Vec<RawEntityLink>>,
    base: &Path,
    check_files: bool,
    diagnostics: &mut Vec<Diagnostic>,
) -> Dag<String, ()> {
    let entity_names: Vec<&str> = entities.keys().map(String::as_str).collect();

    for (id, raw) in entities {
        for (parent, span) in &raw.belong_to {
            if !entities.contains_key(parent) {
                let mut diag = Diagnostic::from_validation(
                    ValidationError::EntityUnknownParent {
                        entity: id.clone(),
                        target: parent.clone(),
                    },
                    Some(*span),
                );
                if let Some(suggestion) = suggest_similar(parent, &entity_names) {
                    diag = diag.with_suggestion(suggestion);
                }
                diagnostics.push(diag);
            }
        }
    }

    for (group, edges) in links {
        for link in edges {
            for (id, span) in [&link.left, &link.right] {
                if !entities.contains_key(id) {
                    let mut diag = Diagnostic::from_validation(
                        ValidationError::UnknownLinkEntity {
                            group: group.clone(),
                            entity: id.clone(),
                        },
                        Some(*span),
                    );
                    if let Some(suggestion) = suggest_similar(id, &entity_names) {
                        diag = diag.with_suggestion(suggestion);
                    }
                    diagnostics.push(diag);
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
            let Some(&parent_idx) = indices.get(parent.as_str()) else {
                continue;
            };
            if graph
                .update_edge(parent_idx, indices[id.as_str()], ())
                .is_err()
            {
                diagnostics.push(Diagnostic::from_validation(
                    ValidationError::EntityCycle { entity: id.clone() },
                    Some(*span),
                ));
            }
        }
    }

    if check_files {
        validate_avatars(entities, base, diagnostics);
    }

    graph
}

fn validate_avatars(
    entities: &BTreeMap<String, RawEntity>,
    base: &Path,
    diagnostics: &mut Vec<Diagnostic>,
) {
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
            diagnostics.push(Diagnostic::from_validation(
                ValidationError::InvalidAvatarPath {
                    entity: id.clone(),
                    path: source.clone(),
                },
                Some(*span),
            ));
        }
    }
}

// ---- 运行时构建（依赖历法与可计算属性脚本） ----

pub(crate) struct EntityBuildContext<'a> {
    pub base: &'a Path,
    pub calendar: &'a Calendar,
    pub calculable_props: &'a CalculablePropRuntime,
}

/// 将静态校验通过的实体表结合历法与可计算属性构建成运行时模型，继续收集运行时错误。
/// 同时返回各实体解析后的基准时间 `Moment`，供事件构建阶段计算相对时间差。
pub(crate) fn build_entities_runtime(
    raw_entities: BTreeMap<String, RawEntity>,
    raw_links: &BTreeMap<String, Vec<RawEntityLink>>,
    belong_graph: Dag<String, ()>,
    context: EntityBuildContext<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) -> (
    HashMap<String, Entity>,
    Vec<BelongNode>,
    HashMap<String, Vec<EntityLink>>,
    HashMap<String, Moment>,
) {
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
    let mut base_time_moments = HashMap::new();
    for (id, input) in raw_entities {
        let belong_to = parents_by_id.get(id.as_str()).cloned().unwrap_or_default();
        let base_time_moment = input
            .base_time
            .as_ref()
            .and_then(|(phase, span)| match context.calendar.resolve(phase) {
                Ok(moment) => Some(moment),
                Err(err) => {
                    diagnostics.push(err.into_diagnostic(Some(*span)));
                    None
                }
            });
        let base_time_humanizer = base_time_moment.as_ref().and_then(|moment| {
            match context.calendar.humanize(moment) {
                Ok(text) => Some(text),
                Err(err) => {
                    diagnostics.push(err.into_diagnostic(input.base_time.as_ref().map(|(_, s)| *s)));
                    None
                }
            }
        });
        let avatar = input
            .avatar
            .map(|(source, _)| source)
            .filter(|source| !source.trim().is_empty())
            .and_then(|source| fs::canonicalize(context.base.join(source)).ok())
            .filter(|path| path.is_file())
            .map(|path| path.to_string_lossy().into_owned());
        let tags = dedup_preserving_order(input.tags.into_iter().map(|(tag, _)| tag));
        let mut entity = Entity {
            id: id.clone(),
            avatar,
            base_time: input.base_time.map(|(phase, _)| phase),
            base_time_humanizer,
            description: input.description,
            tags,
            extra_props: input.extra_props,
        };
        apply_calculable_props(
            &mut entity,
            &input.calculable_props,
            &belong_to,
            base_time_moment.as_ref(),
            &context,
            diagnostics,
        );
        if let Some(moment) = base_time_moment {
            base_time_moments.insert(id.clone(), moment);
        }
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
                        directed: edge.directed,
                    })
                    .collect(),
            )
        })
        .collect();

    (entities, belong_dag, entity_link, base_time_moments)
}

fn apply_calculable_props(
    entity: &mut Entity,
    props: &[(String, ByteSpan)],
    belong_to: &[String],
    base_time_moment: Option<&Moment>,
    context: &EntityBuildContext<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if props.is_empty() {
        return;
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
            "tags": entity.tags,
            "extra_props": raw_extra_props,
        });
        match context.calculable_props.calculable_prop(name, input) {
            Ok(Some(value)) => {
                entity.extra_props.insert(name.clone(), value);
            }
            Ok(None) => {}
            Err(err) => {
                diagnostics.push(err.into_diagnostic(Some(*span)));
            }
        }
    }
}

fn dedup_preserving_order(items: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for item in items {
        if seen.insert(item.clone()) {
            out.push(item);
        }
    }
    out
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
