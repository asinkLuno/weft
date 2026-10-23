//! 实体、归属森林与关系模型：YAML 输入结构、构建上下文与运行时结构，
//! 以及校验、可计算标签展开和图构建的完整流程。

use std::{collections::HashMap, fs, path::Path, sync::Arc};

use serde::{Deserialize, Serialize, Serializer, ser::SerializeStruct};
use serde_json::json;
use serde_yaml_ng::Value;

use crate::calculable_tag::{CalculableTagRuntime, ChainEntry};
use crate::calendar::Calendar;
use crate::weft::FromRaw;
use crate::weft::errors::{DocumentError, ValidationError};
use crate::weft::phase::Phase;

// 归属与可计算标签仅用于输入阶段，构建后统一保存到 EntityForest。
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
    pub calculable_tags: Vec<String>,
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
        })
    }
}

impl Entity {
    // 可计算标签的链上实体：名称即 ID，description 与 base_time 来自条目；
    // base_time 的时间分量与实体输入走同一条历法限制并归一化出展示。
    fn from_chain(entry: &ChainEntry, calendar: &Calendar) -> Result<Self, DocumentError> {
        let base_time = entry.base_time.clone().map(Phase::from_components);
        let base_time_humanizer = base_time
            .as_ref()
            .map(|phase| base_time_humanizer(phase, calendar))
            .transpose()?;
        Ok(Self {
            id: entry.name.clone(),
            avatar: None,
            base_time,
            base_time_humanizer,
            description: entry.description.clone(),
        })
    }
}

// 基准时间的展示：按历法归一化分量后 humanize；实体输入与链上实体共用。
fn base_time_humanizer(phase: &Phase, calendar: &Calendar) -> Result<String, DocumentError> {
    let normalized = calendar.normalize(&phase.flatten(calendar.component_count())?)?;
    calendar.humanize(&normalized)
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

/// 实体集合构建所需的路径、历法和可计算标签环境。
pub(crate) struct EntityCollectionContext<'a> {
    pub base: &'a Path,
    pub calendar: &'a Calendar,
    pub calculable_tags: &'a CalculableTagRuntime,
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

    /// 构建管线：校验输入，RawEntity 转成 Entity，可计算标签展开出链上实体，
    /// 再计算归属森林，最后把 RawEntityLink 转成 EntityLink。
    pub fn build(
        mut raw_entities: HashMap<String, RawEntity>,
        raw_links: HashMap<String, Vec<RawEntityLink>>,
        context: EntityCollectionContext<'_>,
    ) -> Result<Self, DocumentError> {
        let EntityCollectionContext {
            base,
            calendar,
            calculable_tags,
            date_mode,
            lang,
        } = context;
        // 归属边与标签声明属于输入侧，先拆出单独保存。
        let mut belong_to = HashMap::new();
        let mut tags = HashMap::new();
        for (id, input) in &mut raw_entities {
            belong_to.insert(id.clone(), std::mem::take(&mut input.belong_to));
            tags.insert(id.clone(), std::mem::take(&mut input.calculable_tags));
        }
        validate(&belong_to, &raw_links)?;
        validate_avatars(&raw_entities, base)?;
        let mut entities = HashMap::new();
        for (id, input) in raw_entities {
            let entity = Entity::from_raw(
                input,
                EntityContext {
                    id: id.clone(),
                    calendar,
                    base,
                },
            )?;
            entities.insert(id, Arc::new(entity));
        }
        // 先根据可计算标签创建链上实体并并入归属边，链首撞到已有归属可能成环，复检后再建树。
        apply_calculable_tags(
            &mut entities,
            &mut belong_to,
            &tags,
            calculable_tags,
            date_mode,
            lang,
            calendar,
        )?;
        check_cycles(&belong_to)?;
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

// 执行可计算标签，为链上尚不存在的 ID 直接创建实体并并入归属边。
fn apply_calculable_tags(
    entities: &mut HashMap<String, Arc<Entity>>,
    belong_to: &mut HashMap<String, Vec<String>>,
    tags: &HashMap<String, Vec<String>>,
    runtime: &CalculableTagRuntime,
    mode: Option<&str>,
    lang: &str,
    cal: &Calendar,
) -> Result<(), DocumentError> {
    let mut ids: Vec<_> = tags.keys().cloned().collect();
    ids.sort();
    for id in ids {
        for name in &tags[&id] {
            let entity = &entities[&id];
            let base_time = entity
                .base_time
                .as_ref()
                .map(|phase| cal.normalize(&phase.flatten(cal.component_count())?))
                .transpose()?;
            let parents = &belong_to[&id];
            let context = json!({
                "api": 1, "date_mode": mode, "language": lang,
                "entity": { "id": id, "base_time": base_time, "description": entity.description, "belong_to": parents },
            });
            let chain = runtime.calculable_tag(name, &context, lang)?;
            apply_chain(entities, belong_to, &id, &chain, cal)?;
        }
    }
    Ok(())
}

// 链首并入源实体的归属；链上撞到已有实体就停止创建，也不改动它的归属。
fn apply_chain(
    entities: &mut HashMap<String, Arc<Entity>>,
    belong_to: &mut HashMap<String, Vec<String>>,
    id: &str,
    chain: &[ChainEntry],
    cal: &Calendar,
) -> Result<(), DocumentError> {
    let Some(first) = chain.first() else {
        return Ok(());
    };
    let existing: Vec<_> = chain
        .iter()
        .map(|entry| entities.contains_key(&entry.name))
        .collect();
    for (i, entry) in chain.iter().enumerate() {
        if existing[i] {
            break;
        }
        let entity = Entity::from_chain(entry, cal)?;
        let next = chain
            .get(i + 1)
            .map(|entry| entry.name.clone())
            .into_iter()
            .collect();
        entities.insert(entry.name.clone(), Arc::new(entity));
        belong_to.insert(entry.name.clone(), next);
    }
    let owner = belong_to.get_mut(id).expect("可计算标签的源实体存在");
    if !owner.contains(&first.name) {
        owner.push(first.name.clone());
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

// 归属边成环检查；输入校验与可计算标签展开后各跑一次。
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
