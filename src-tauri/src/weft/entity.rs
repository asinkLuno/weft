//! 实体、归属森林与关系模型：YAML 输入结构、构建上下文与运行时结构，
//! 以及校验、可计算属性执行和图构建的完整流程。

use std::{collections::HashMap, fs, path::Path, sync::Arc};

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
            .map(|phase| base_time_humanizer(phase, calendar))
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
                .iter()
                .map(|(key, value)| (key.clone(), yaml_to_json(value)))
                .collect(),
        })
    }
}

// 基准时间的展示：按历法归一化分量后 humanize。
fn base_time_humanizer(phase: &Phase, calendar: &Calendar) -> Result<String, DocumentError> {
    let normalized = calendar.normalize(&phase.flatten(calendar.component_count())?)?;
    calendar.humanize(&normalized)
}

// extra_props 原样透传给前端：YAML 值转成 JSON，非字符串的映射键转成字符串，
// 标签值（!tag）只保留内容，非有限浮点按 JSON 惯例落为 null。
fn yaml_to_json(value: &Value) -> JsonValue {
    match value {
        Value::Null => JsonValue::Null,
        Value::Bool(boolean) => (*boolean).into(),
        Value::Number(number) => number
            .as_i64()
            .map(|i| i.into())
            .or_else(|| number.as_u64().map(|u| u.into()))
            .unwrap_or_else(|| number.as_f64().into()),
        Value::String(string) => string.clone().into(),
        Value::Sequence(sequence) => sequence.iter().map(yaml_to_json).collect(),
        Value::Mapping(mapping) => mapping
            .iter()
            .map(|(key, value)| (yaml_key_to_string(key), yaml_to_json(value)))
            .collect(),
        Value::Tagged(tagged) => yaml_to_json(&tagged.value),
    }
}

fn yaml_key_to_string(key: &Value) -> String {
    match key {
        Value::String(string) => string.clone(),
        key => yaml_to_json(key).to_string(),
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
        mut raw_entities: HashMap<String, RawEntity>,
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
        // 归属边与属性声明属于输入侧，先拆出单独保存。
        let mut belong_to = HashMap::new();
        let mut props = HashMap::new();
        for (id, input) in &mut raw_entities {
            belong_to.insert(id.clone(), std::mem::take(&mut input.belong_to));
            props.insert(id.clone(), std::mem::take(&mut input.calculable_props));
        }
        validate(&belong_to, &raw_links)?;
        validate_avatars(&raw_entities, base)?;
        let mut entities = HashMap::new();
        for (id, input) in raw_entities {
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
                &belong_to[&id],
                &props[&id],
                calculable_props,
                date_mode,
                lang,
                calendar,
            )?;
            entities.insert(id, Arc::new(entity));
        }
        let entity_forest = build_forest(&belong_to, &entities);
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

// 将已校验的归属关系展开为森林，节点只引用实体数据。
// 引用存在且无环已由集合校验；根节点和同级子节点按 ID 排序。
fn build_forest(
    belong_to: &HashMap<String, Vec<String>>,
    entities: &HashMap<String, Arc<Entity>>,
) -> EntityForest {
    let mut roots = Vec::new();
    let mut children: HashMap<&str, Vec<&str>> = HashMap::new();
    for (id, parents) in belong_to {
        if parents.is_empty() {
            roots.push(id.as_str());
        }
        for parent in parents {
            children.entry(parent).or_default().push(id);
        }
    }
    roots.sort_unstable();
    for ids in children.values_mut() {
        ids.sort_unstable();
        ids.dedup();
    }
    roots
        .into_iter()
        .map(|id| build_tree(id, &children, entities))
        .collect()
}

fn build_tree(
    id: &str,
    children: &HashMap<&str, Vec<&str>>,
    entities: &HashMap<String, Arc<Entity>>,
) -> EntityBelongTo {
    EntityBelongTo {
        entity: Arc::clone(&entities[id]),
        children: children
            .get(id)
            .into_iter()
            .flatten()
            .map(|child| build_tree(child, children, entities))
            .collect(),
    }
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

fn validate(
    belong_to: &HashMap<String, Vec<String>>,
    links: &HashMap<String, Vec<RawEntityLink>>,
) -> Result<(), DocumentError> {
    let mut errors = Vec::new();
    for (id, parents) in belong_to {
        for parent in parents {
            if !belong_to.contains_key(parent) {
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
                if !belong_to.contains_key(id) {
                    errors.push(ValidationError::UnknownLinkEntity {
                        group: group.clone(),
                        entity: id.clone(),
                    });
                }
            }
        }
    }
    report(errors)?;
    check_cycles(belong_to)
}

// 归属边成环检查；构建前对输入归属跑一次。
fn check_cycles(belong_to: &HashMap<String, Vec<String>>) -> Result<(), DocumentError> {
    let mut state = HashMap::new();
    for id in belong_to.keys() {
        visit(id, belong_to, &mut state)?;
    }
    Ok(())
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

#[derive(Clone, Copy)]
enum VisitState {
    Visiting,
    Visited,
}

fn visit<'a>(
    id: &'a str,
    graph: &'a HashMap<String, Vec<String>>,
    state: &mut HashMap<&'a str, VisitState>,
) -> Result<(), DocumentError> {
    match state.get(id) {
        Some(VisitState::Visiting) => {
            return Err(ValidationError::EntityCycle {
                entity: id.to_owned(),
            }
            .into());
        }
        Some(VisitState::Visited) => return Ok(()),
        _ => {}
    }
    state.insert(id, VisitState::Visiting);
    for parent in &graph[id] {
        visit(parent, graph, state)?;
    }
    state.insert(id, VisitState::Visited);
    Ok(())
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
