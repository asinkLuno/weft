//! 实体、归属图与关系模型：YAML 输入结构、构建上下文与运行时结构，
//! 以及校验、可计算属性执行和图构建的完整流程。

use std::{collections::HashMap, fs, path::Path, sync::Arc};

use daggy::{Dag, Walker};
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use serde_yaml_ng::Value;

use crate::calculable_prop::CalculablePropRuntime;
use crate::calendar::Calendar;
use crate::weft::FromRaw;
use crate::weft::errors::{DocumentError, ValidationError};
use crate::weft::phase::Phase;

// ---- 输入结构 ----

// 归属与可计算属性声明仅用于输入阶段，归属校验后反查写回实体。
// base_time 保留原始 YAML 值，构建时经 Phase::from_yaml_scalar_opt 解析。
#[derive(Debug, Deserialize)]
pub(crate) struct RawEntity {
    #[serde(default)]
    pub base_time: Option<Value>,
    pub description: Option<String>,
    pub avatar: Option<String>,
    #[serde(default)]
    pub belong_to: Vec<String>,
    #[serde(default)]
    pub calculable_props: Vec<String>,
    // 内置 key 之外的一切输入字段原样收进 extra_props。
    #[serde(flatten)]
    pub extra_props: HashMap<String, Value>,
}

// ---- 实体 ----

/// 实体构建所需的 ID、直接归属、历法和 YAML 基准路径。
struct EntityContext<'a> {
    id: String,
    belong_to: Vec<String>,
    calendar: &'a Calendar,
    base: &'a Path,
}

/// 已完成头像解析和基准时间展示归一化的实体；顶层实体表和事件共享同一份数据。
#[derive(Debug, Serialize)]
pub struct Entity {
    pub id: String,
    // 直接上级实体 ID，由归属 DAG 反查得出，已排序去重；无归属时省略。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub belong_to: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    // 原始 Phase 用于偏移运算，展示归一化不会改写它。
    pub base_time: Option<Phase>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_time_humanizer: Option<String>,
    pub description: Option<String>,
    // 非内置输入字段转成 JSON 后透传，为空时序列化中省略。
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub extra_props: HashMap<String, JsonValue>,
}

impl FromRaw<EntityContext<'_>> for Entity {
    type Raw = RawEntity;

    fn from_raw(raw: Self::Raw, context: EntityContext<'_>) -> Result<Self, DocumentError> {
        let EntityContext {
            id,
            belong_to,
            calendar,
            base,
        } = context;
        let base_time = Phase::from_yaml_scalar_opt(raw.base_time.as_ref())?;
        let base_time_humanizer = base_time
            .as_ref()
            .map(|phase| {
                let normalized = calendar.normalize(&phase.flatten(calendar.component_count())?)?;
                calendar.humanize(&normalized)
            })
            .transpose()?;
        // 头像路径相对 YAML 文件，是否为存在的图片已在输入校验确认，此处仅解析绝对路径。
        let avatar = raw
            .avatar
            .filter(|source| !source.trim().is_empty())
            .and_then(|source| fs::canonicalize(base.join(source)).ok())
            .filter(|path| path.is_file())
            .map(|path| path.to_string_lossy().into_owned());
        Ok(Self {
            id,
            belong_to,
            avatar,
            base_time,
            base_time_humanizer,
            description: raw.description,
            extra_props: raw
                .extra_props
                .into_iter()
                .map(|(key, value)| serde_json::to_value(value).map(|value| (key, value)))
                .collect::<Result<_, _>>()
                .map_err(DocumentError::raw)?,
        })
    }
}

/// 实体 ID → 已构建实体；顶层实体表与事件共享同一份引用。
pub type EntityMap = HashMap<String, Arc<Entity>>;

// ---- 关系 ----

/// 一条关系：两个实体 ID 与关系类型；ID 是否合法由构建管线统一校验。
#[derive(Debug, Deserialize, Serialize)]
pub struct EntityLink {
    pub entities: (String, String),
    pub relations: String,
}

/// 关系组名 → 该组的关系列表。
pub type EntityLinks = HashMap<String, Vec<EntityLink>>;

// ---- 构建管线 ----

/// 实体段构建所需的路径、历法和可计算属性环境。
pub(crate) struct EntityBuildContext<'a> {
    pub base: &'a Path,
    pub calendar: &'a Calendar,
    pub calculable_props: &'a CalculablePropRuntime,
    pub date_mode: Option<&'a str>,
    pub lang: &'a str,
}

/// 构建实体段：校验输入，RawEntity 转成 Entity 并执行可计算属性，
/// 结果写进 extra_props。归属 DAG 的边已去重且无环，反查出的直接上级随实体序列化。
/// 关系只校验 ID，原样保留。
pub(crate) fn build(
    raw_entities: HashMap<String, RawEntity>,
    entity_link: EntityLinks,
    context: EntityBuildContext<'_>,
) -> Result<(EntityMap, EntityLinks), DocumentError> {
    let EntityBuildContext {
        base,
        calendar,
        calculable_props,
        date_mode,
        lang,
    } = context;
    let belong_graph = build_belong_graph(&raw_entities, &entity_link)?;
    validate_avatars(&raw_entities, base)?;
    let mut parents_by_id: HashMap<&str, Vec<String>> = belong_graph
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
    let mut entities = HashMap::new();
    for (id, mut input) in raw_entities {
        let props = std::mem::take(&mut input.calculable_props);
        let belong_to = parents_by_id.remove(id.as_str()).unwrap_or_default();
        let mut entity = Entity::from_raw(
            input,
            EntityContext {
                id: id.clone(),
                belong_to,
                calendar,
                base,
            },
        )?;
        apply_calculable_props(
            &mut entity,
            &props,
            calculable_props,
            date_mode,
            lang,
            calendar,
        )?;
        entities.insert(id, Arc::new(entity));
    }
    Ok((entities, entity_link))
}

fn build_belong_graph(
    entities: &HashMap<String, RawEntity>,
    links: &EntityLinks,
) -> Result<Dag<String, ()>, DocumentError> {
    let mut errors = Vec::new();
    for (id, input) in entities {
        for parent in &input.belong_to {
            if !entities.contains_key(parent) {
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
                if !entities.contains_key(id) {
                    errors.push(ValidationError::UnknownLinkEntity {
                        group: group.clone(),
                        entity: id.clone(),
                    });
                }
            }
        }
    }
    report(errors)?;
    let mut graph = Dag::new();
    let mut ids: Vec<_> = entities.keys().collect();
    ids.sort_unstable();
    let indices: HashMap<_, _> = ids
        .iter()
        .map(|id| (id.as_str(), graph.add_node((*id).clone())))
        .collect();
    for id in ids {
        for parent in &entities[id].belong_to {
            graph
                .update_edge(indices[parent.as_str()], indices[id.as_str()], ())
                .map_err(|_| ValidationError::EntityCycle { entity: id.clone() })?;
        }
    }
    Ok(graph)
}

// 头像路径相对 YAML 文件；必须解析到已存在的图片文件，否则逐条报错。
fn validate_avatars(entity: &HashMap<String, RawEntity>, base: &Path) -> Result<(), DocumentError> {
    let mut errors = Vec::new();
    for (id, input) in entity {
        let Some(source) = input
            .avatar
            .as_deref()
            .filter(|source| !source.trim().is_empty())
        else {
            continue;
        };
        let valid = fs::canonicalize(base.join(source))
            .map(|path| path.is_file() && is_image(&path))
            .unwrap_or(false);
        if !valid {
            errors.push(ValidationError::InvalidAvatarPath {
                entity: id.clone(),
                path: source.to_owned(),
            });
        }
    }
    report(errors)
}

// 执行实体的可计算属性：脚本返回值写进 extra_props 的属性注册 ID 下，
// 与输入字段同名时以计算结果为准；返回 () 则不写入。
fn apply_calculable_props(
    entity: &mut Entity,
    props: &[String],
    runtime: &CalculablePropRuntime,
    mode: Option<&str>,
    lang: &str,
    cal: &Calendar,
) -> Result<(), DocumentError> {
    for name in props {
        let base_time = entity
            .base_time
            .as_ref()
            .map(|phase| cal.normalize(&phase.flatten(cal.component_count())?))
            .transpose()?;
        let context = json!({
            "api": 1, "date_mode": mode, "language": lang,
            "entity": { "id": entity.id, "base_time": base_time, "description": entity.description, "belong_to": entity.belong_to },
        });
        if let Some(value) = runtime.calculable_prop(name, &context)? {
            entity.extra_props.insert(name.clone(), value);
        }
    }
    Ok(())
}

// ---- 校验工具 ----

// 有错误时排序汇总为一条 References，一次暴露全部问题。
fn report(mut errors: Vec<ValidationError>) -> Result<(), DocumentError> {
    if errors.is_empty() {
        return Ok(());
    }
    errors.sort_by_cached_key(ToString::to_string);
    Err(DocumentError::References { errors })
}

// 头像经 asset protocol 直接交给 <img>，按扩展名接受 WebView 可渲染的图片。
const IMAGE_EXTENSIONS: [&str; 8] = ["bmp", "gif", "ico", "jpeg", "jpg", "png", "svg", "webp"];

fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            IMAGE_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
        })
}
