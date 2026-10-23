//! 实体、归属森林与关系模型：YAML 输入结构、构建上下文与运行时结构，
//! 以及校验、可计算属性执行和图构建的完整流程。

use std::{collections::HashMap, fs, path::Path, sync::Arc};

use daggy::{Dag, NodeIndex, Walker};
use serde::{Deserialize, Serialize, Serializer, ser::SerializeStruct};
use serde_json::{Value as JsonValue, json};
use serde_yaml_ng::Value;

use crate::calculable_prop::CalculablePropRuntime;
use crate::calendar::Calendar;
use crate::weft::FromRaw;
use crate::weft::errors::{DocumentError, ValidationError};
use crate::weft::phase::Phase;

// 归属与可计算属性声明仅用于输入阶段，归属构建后统一保存到 EntityForest。
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

#[derive(Debug, Deserialize)]
pub(crate) struct RawEntityLink {
    pub entities: (String, String),
    pub relations: String,
}

/// 实体构建所需的 ID、历法和 YAML 基准路径。
struct EntityContext<'a> {
    id: String,
    calendar: &'a Calendar,
    base: &'a Path,
}

/// 已完成头像解析和基准时间展示归一化的实体；集合、森林、关系和事件共享同一份数据。
#[derive(Debug, Serialize)]
pub struct Entity {
    pub id: String,
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
        let EntityContext { id, calendar, base } = context;
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

/// 归属树节点：父实体包含直接归属于它的子节点。
#[derive(Debug, Serialize)]
pub struct EntityBelongTo {
    #[serde(serialize_with = "serialize_entity")]
    pub entity: Arc<Entity>,
    pub children: EntityForest,
}

/// 以无归属实体为根的树集合；多重归属的实体出现在各个父节点下。
pub type EntityForest = Vec<EntityBelongTo>;

/// 关系构建所需的关系组名和已构建实体表。
struct EntityLinkContext<'a> {
    group: &'a str,
    entities: &'a HashMap<String, Arc<Entity>>,
}

/// 端点已解析为共享实体引用的关系。
#[derive(Debug, Serialize)]
pub struct EntityLink {
    #[serde(serialize_with = "serialize_endpoints")]
    pub entities: (Arc<Entity>, Arc<Entity>),
    pub relations: String,
}

impl FromRaw<EntityLinkContext<'_>> for EntityLink {
    type Raw = RawEntityLink;

    fn from_raw(raw: Self::Raw, context: EntityLinkContext<'_>) -> Result<Self, DocumentError> {
        let resolve = |id: &str| {
            context
                .entities
                .get(id)
                .map(Arc::clone)
                .ok_or_else(|| DocumentError::References {
                    errors: vec![ValidationError::UnknownLinkEntity {
                        group: context.group.to_owned(),
                        entity: id.to_owned(),
                    }],
                })
        };
        Ok(Self {
            entities: (resolve(&raw.entities.0)?, resolve(&raw.entities.1)?),
            relations: raw.relations,
        })
    }
}

/// 实体集合构建所需的路径、历法和可计算属性环境。
pub(crate) struct EntityCollectionContext<'a> {
    pub base: &'a Path,
    pub calendar: &'a Calendar,
    pub calculable_props: &'a CalculablePropRuntime,
    pub date_mode: Option<&'a str>,
    pub lang: &'a str,
}

/// 已完成校验、标签展开和图构建的实体集合。
#[derive(Debug)]
pub struct EntityCollection {
    entities: HashMap<String, Arc<Entity>>,
    pub entity_forest: EntityForest,
    pub entity_link: HashMap<String, Vec<EntityLink>>,
}

impl Serialize for EntityCollection {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let entities: HashMap<_, _> = self
            .entities
            .iter()
            .map(|(id, entity)| (id, entity.as_ref()))
            .collect();
        let mut state = serializer.serialize_struct("EntityCollection", 3)?;
        state.serialize_field("entity", &entities)?;
        state.serialize_field("entity_forest", &self.entity_forest)?;
        state.serialize_field("entity_link", &self.entity_link)?;
        state.end()
    }
}

impl EntityCollection {
    pub fn as_map(&self) -> &HashMap<String, Arc<Entity>> {
        &self.entities
    }

    /// 构建管线：校验输入，RawEntity 转成 Entity 并执行可计算属性，
    /// 结果写进 extra_props，最后计算归属森林并构建关系。
    pub fn build(
        raw_entities: HashMap<String, RawEntity>,
        raw_links: HashMap<String, Vec<RawEntityLink>>,
        context: EntityCollectionContext<'_>,
    ) -> Result<Self, DocumentError> {
        let EntityCollectionContext {
            base,
            calendar,
            calculable_props,
            date_mode,
            lang,
        } = context;
        let belong_graph = build_belong_graph(&raw_entities, &raw_links)?;
        validate_avatars(&raw_entities, base)?;
        let mut entities = HashMap::new();
        for (id, mut input) in raw_entities {
            let props = std::mem::take(&mut input.calculable_props);
            let parents = std::mem::take(&mut input.belong_to);
            let mut entity = Entity::from_raw(
                input,
                EntityContext {
                    id: id.clone(),
                    calendar,
                    base,
                },
            )?;
            apply_calculable_props(
                &mut entity,
                &parents,
                &props,
                calculable_props,
                date_mode,
                lang,
                calendar,
            )?;
            entities.insert(id, Arc::new(entity));
        }
        let entity_forest = build_forest(&belong_graph, &entities);
        let mut entity_link = HashMap::new();
        for (group, links) in raw_links {
            let links = links
                .into_iter()
                .map(|raw| {
                    EntityLink::from_raw(
                        raw,
                        EntityLinkContext {
                            group: &group,
                            entities: &entities,
                        },
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            entity_link.insert(group, links);
        }
        Ok(Self {
            entities,
            entity_forest,
            entity_link,
        })
    }
}

// DAG 中父节点指向子节点；展开时按 ID 排序，多重归属出现在各个分支下。
fn build_forest(graph: &Dag<String, ()>, entities: &HashMap<String, Arc<Entity>>) -> EntityForest {
    fn build_tree(
        node: NodeIndex,
        graph: &Dag<String, ()>,
        entities: &HashMap<String, Arc<Entity>>,
    ) -> EntityBelongTo {
        let mut children: Vec<_> = graph
            .children(node)
            .iter(graph)
            .map(|(_, child)| child)
            .collect();
        children.sort_unstable_by_key(|&child| &graph[child]);
        EntityBelongTo {
            entity: Arc::clone(&entities[&graph[node]]),
            children: children
                .into_iter()
                .map(|child| build_tree(child, graph, entities))
                .collect(),
        }
    }

    let mut roots: Vec<_> = graph
        .graph()
        .node_indices()
        .filter(|&node| graph.parents(node).iter(graph).next().is_none())
        .collect();
    roots.sort_unstable_by_key(|&node| &graph[node]);
    roots
        .into_iter()
        .map(|node| build_tree(node, graph, entities))
        .collect()
}

// 执行实体的可计算属性：脚本返回值写进 extra_props 的属性注册 ID 下，
// 与输入字段同名时以计算结果为准；返回 () 则不写入。
fn apply_calculable_props(
    entity: &mut Entity,
    parents: &[String],
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
            "entity": { "id": entity.id, "base_time": base_time, "description": entity.description, "belong_to": parents },
        });
        if let Some(value) = runtime.calculable_prop(name, &context)? {
            entity.extra_props.insert(name.clone(), value);
        }
    }
    Ok(())
}

// 实体引用、头像路径与归属图的完整性校验。
// 头像经 asset protocol 直接交给 <img>，按扩展名接受 WebView 可渲染的图片。
const IMAGE_EXTENSIONS: [&str; 8] = ["bmp", "gif", "ico", "jpeg", "jpg", "png", "svg", "webp"];

fn build_belong_graph(
    entities: &HashMap<String, RawEntity>,
    links: &HashMap<String, Vec<RawEntityLink>>,
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

// 有错误时排序汇总为一条 References，一次暴露全部问题。
fn report(mut errors: Vec<ValidationError>) -> Result<(), DocumentError> {
    if errors.is_empty() {
        return Ok(());
    }
    errors.sort_by_cached_key(ToString::to_string);
    Err(DocumentError::References { errors })
}

fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            IMAGE_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
        })
}

fn serialize_entity<S: Serializer>(entity: &Entity, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&entity.id)
}

fn serialize_endpoints<S: Serializer>(
    entities: &(Arc<Entity>, Arc<Entity>),
    serializer: S,
) -> Result<S::Ok, S::Error> {
    (&entities.0.id, &entities.1.id).serialize(serializer)
}
