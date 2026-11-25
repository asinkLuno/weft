//! `.weft` 编译器管线：AST 降级、时间引用解析、语义校验与运行时 `WeftDoc` 构建。

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::Path,
};

use serde::Serialize;
use serde_json::{Map as JsonMap, Number as JsonNumber, Value as JsonValue};

use crate::weft::entity::{
    BelongNode, Entity, EntityBuildContext, EntityLink, RawEntity, RawEntityLink, build_entities,
};
use crate::weft::errors::{DocumentError, SourceSpan, ValidationError};
use crate::weft::event::{Event, RawEvent, build_events, check as check_event};
use crate::weft::narrative::{Narrative, check as check_narrative};
use crate::weft::parser::{Attr, Expr, Span, Spanned, TopItem, WeftAst, parse_weft, source_span};
use crate::weft::phase::{TimeResolver, TimeTarget};
use crate::weft::story::{RawStory, Story, StoryContext};

/// 编译后的运行时故事文档，直接序列化为 JSON 交付前端。
#[derive(Debug, Serialize)]
pub struct WeftDoc {
    pub story: Story,
    pub entity: HashMap<String, Entity>,
    pub belong_dag: Vec<BelongNode>,
    pub entity_link: HashMap<String, Vec<EntityLink>>,
    pub event: BTreeMap<String, Event>,
    pub narrative: BTreeMap<String, Narrative>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calendar_components: Option<usize>,
}

/// 从 `story` 块提取的脚本依赖路径，供文件监听器跟踪 `.rn` 脚本变更。
pub struct StoryScriptRefs {
    pub date_mode: Option<String>,
    pub calculable_props: Vec<String>,
}

/// 单遍解析 `.weft`：同时返回从 `story` 块提取的脚本依赖（即便后续语义校验或脚本加载失败）与编译结果。
pub fn compile_with_scripts(
    src: &str,
    base: &Path,
    gregorian: &Path,
    lang: &str,
) -> (Option<StoryScriptRefs>, Result<WeftDoc, DocumentError>) {
    let ast = match parse_weft(src) {
        Ok(ast) => ast,
        Err(err) => return (None, Err(err)),
    };
    let scripts = extract_script_refs(&ast);
    let result = compile_ast(src, ast, base, gregorian, lang);
    (scripts, result)
}

fn extract_script_refs(ast: &WeftAst) -> Option<StoryScriptRefs> {
    for (item, _) in &ast.items {
        if let TopItem::Story(attrs) = item {
            let mut date_mode = None;
            let mut calculable_props = Vec::new();
            for (attr, _) in attrs {
                match attr.key.0.as_str() {
                    "date_mode" => {
                        if let Expr::Str(s) = &attr.value.0 {
                            date_mode = Some(s.clone());
                        }
                    }
                    "calculable_props" => {
                        if let Expr::Object(entries) = &attr.value.0 {
                            for (entry, _) in entries {
                                if let Expr::Str(path) = &entry.value.0 {
                                    calculable_props.push(path.clone());
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            return Some(StoryScriptRefs {
                date_mode,
                calculable_props,
            });
        }
    }
    None
}

fn compile_ast(
    src: &str,
    ast: WeftAst,
    base: &Path,
    gregorian: &Path,
    lang: &str,
) -> Result<WeftDoc, DocumentError> {
    let mut story_attrs: Option<(Vec<Spanned<Attr>>, Span)> = None;
    let mut anchor_names: BTreeSet<String> = BTreeSet::new();
    let mut time_exprs: HashMap<TimeTarget, Spanned<Expr>> = HashMap::new();
    let mut entity_blocks: BTreeMap<String, Vec<Spanned<Attr>>> = BTreeMap::new();
    let mut link_groups: BTreeMap<String, Vec<RawEntityLink>> = BTreeMap::new();
    let mut event_blocks: BTreeMap<String, (Vec<Spanned<Attr>>, Span)> = BTreeMap::new();
    let mut raw_narratives: BTreeMap<String, Vec<(String, SourceSpan)>> = BTreeMap::new();

    for (item, item_span) in ast.items {
        match item {
            TopItem::Story(attrs) => {
                if story_attrs.is_some() {
                    return Err(DocumentError::validation_at(
                        ValidationError::DuplicateStory,
                        source_span(src, item_span),
                    ));
                }
                story_attrs = Some((attrs, item_span));
            }
            TopItem::TimeAnchor { name, expr } => {
                if !anchor_names.insert(name.0.clone()) {
                    return Err(DocumentError::validation_at(
                        ValidationError::DuplicateTimeAnchor { name: name.0 },
                        source_span(src, name.1),
                    ));
                }
                time_exprs.insert(TimeTarget::Anchor(name.0), expr);
            }
            TopItem::Entity { name, attrs } => {
                if entity_blocks.contains_key(&name.0) {
                    return Err(DocumentError::validation_at(
                        ValidationError::DuplicateEntity { entity: name.0 },
                        source_span(src, name.1),
                    ));
                }
                entity_blocks.insert(name.0, attrs);
            }
            TopItem::Link { group, edges } => {
                let entry = link_groups.entry(group.0).or_default();
                for (edge, _) in edges {
                    entry.push(RawEntityLink {
                        left: (edge.left.0, source_span(src, edge.left.1)),
                        right: (edge.right.0, source_span(src, edge.right.1)),
                        relations: edge.relation.0,
                    });
                }
            }
            TopItem::Event { name, attrs } => {
                if event_blocks.contains_key(&name.0) {
                    return Err(DocumentError::validation_at(
                        ValidationError::DuplicateEvent { event: name.0 },
                        source_span(src, name.1),
                    ));
                }
                event_blocks.insert(name.0, (attrs, item_span));
            }
            TopItem::Narrative { name, events } => {
                if raw_narratives.contains_key(&name.0) {
                    return Err(DocumentError::validation_at(
                        ValidationError::DuplicateNarrative { narrative: name.0 },
                        source_span(src, name.1),
                    ));
                }
                let refs = events
                    .into_iter()
                    .map(|(ev, sp)| (ev, source_span(src, sp)))
                    .collect();
                raw_narratives.insert(name.0, refs);
            }
        }
    }

    let (story_attrs, story_span) = story_attrs.ok_or(ValidationError::MissingStory)?;
    let raw_story = lower_story(src, story_attrs, story_span)?;

    // 收集实体/事件字段并把所有时间表达式汇入同一张 `time_exprs` 表，交由统一 TimeResolver 展开。
    let mut raw_entities: BTreeMap<String, RawEntity> = BTreeMap::new();
    for (id, attrs) in entity_blocks {
        let (raw, base_time_expr) = lower_entity(src, &id, attrs)?;
        if let Some(expr) = base_time_expr {
            time_exprs.insert(TimeTarget::EntityBaseTime(id.clone()), expr);
        }
        raw_entities.insert(id, raw);
    }

    let mut pending_events: BTreeMap<String, (Vec<(String, SourceSpan)>, Option<String>)> =
        BTreeMap::new();
    for (id, (attrs, ev_span)) in event_blocks {
        let (entities, description, start_expr, end_expr) =
            lower_event(src, &id, attrs, ev_span)?;
        time_exprs.insert(TimeTarget::EventStart(id.clone()), start_expr);
        if let Some(end_expr) = end_expr {
            time_exprs.insert(TimeTarget::EventEnd(id.clone()), end_expr);
        }
        pending_events.insert(id, (entities, description));
    }

    let mut resolver = TimeResolver::new(src, &time_exprs);

    // 校验所有独立声明的 time 锚点（即使未被事件引用也提前捕获错误）。
    for name in anchor_names {
        let target = TimeTarget::Anchor(name);
        let span = time_exprs[&target].1;
        resolver.resolve_target(&target, span)?;
    }

    for (id, raw) in &mut raw_entities {
        let target = TimeTarget::EntityBaseTime(id.clone());
        if let Some(expr) = time_exprs.get(&target) {
            let phase = resolver.resolve_target(&target, expr.1)?;
            raw.base_time = Some((phase, source_span(src, expr.1)));
        }
    }

    let mut raw_events: BTreeMap<String, RawEvent> = BTreeMap::new();
    for (id, (entities, description)) in pending_events {
        let start_target = TimeTarget::EventStart(id.clone());
        let start_span = time_exprs[&start_target].1;
        let start_phase = resolver.resolve_target(&start_target, start_span)?;

        let end_target = TimeTarget::EventEnd(id.clone());
        let end_time = match time_exprs.get(&end_target) {
            Some(expr) => {
                let phase = resolver.resolve_target(&end_target, expr.1)?;
                Some((phase, source_span(src, expr.1)))
            }
            None => None,
        };
        raw_events.insert(
            id,
            RawEvent {
                start_time: (start_phase, source_span(src, start_span)),
                end_time,
                entities,
                description,
            },
        );
    }

    // 按段校验与构建：story → entity → event → narrative
    let story = Story::build(
        raw_story,
        StoryContext {
            base,
            gregorian,
            lang,
        },
    )?;
    let (entity, belong_dag, entity_link) = build_entities(
        raw_entities,
        &link_groups,
        EntityBuildContext {
            base,
            calendar: &story.calendar,
            calculable_props: &story.calculable_prop_runtime,
        },
    )?;
    check_event(&raw_events, &entity)?;
    check_narrative(&raw_narratives, &raw_events)?;

    let narrative: BTreeMap<String, Narrative> = raw_narratives
        .into_iter()
        .map(|(name, refs)| (name, refs.into_iter().map(|(id, _)| id).collect()))
        .collect();
    let calendar_components = Some(story.calendar.component_count());
    let event = build_events(raw_events, &story.calendar)?;

    Ok(WeftDoc {
        story,
        entity,
        belong_dag,
        entity_link,
        event,
        narrative,
        calendar_components,
    })
}

// ---- AST Lowering ----

fn lower_story(
    src: &str,
    attrs: Vec<Spanned<Attr>>,
    block_span: Span,
) -> Result<RawStory, DocumentError> {
    let section = "story";
    let mut title = None;
    let mut description = None;
    let mut author = None;
    let mut date_mode = None;
    let mut calculable_props = HashMap::new();

    for (attr, _) in attrs {
        let key = attr.key.0;
        let key_span = attr.key.1;
        match key.as_str() {
            "title" => title = Some(expect_string(src, section, &key, attr.value)?),
            "description" => {
                description = expect_opt_string(src, section, &key, attr.value)?;
            }
            "author" => {
                author = expect_opt_string(src, section, &key, attr.value)?;
            }
            "date_mode" => {
                date_mode = expect_opt_string(src, section, &key, attr.value)?;
            }
            "calculable_props" => {
                calculable_props = expect_string_map(src, section, &key, attr.value)?;
            }
            _ => {
                return Err(DocumentError::validation_at(
                    ValidationError::UnknownField {
                        section: section.to_owned(),
                        field: key,
                    },
                    source_span(src, key_span),
                ));
            }
        }
    }

    let title = title.ok_or_else(|| {
        DocumentError::validation_at(
            ValidationError::MissingField {
                section: section.to_owned(),
                field: "title".to_owned(),
            },
            source_span(src, block_span),
        )
    })?;

    Ok(RawStory {
        title,
        description,
        author,
        date_mode,
        calculable_props,
    })
}

fn lower_entity(
    src: &str,
    id: &str,
    attrs: Vec<Spanned<Attr>>,
) -> Result<(RawEntity, Option<Spanned<Expr>>), DocumentError> {
    let section = format!("entity `{id}`");
    let mut base_time = None;
    let mut description = None;
    let mut avatar = None;
    let mut belong_to = Vec::new();
    let mut calculable_props = Vec::new();
    let mut extra_props = HashMap::new();

    for (attr, _) in attrs {
        let key = attr.key.0;
        match key.as_str() {
            "base_time" => {
                if !matches!(attr.value.0, Expr::Null) {
                    base_time = Some(attr.value);
                }
            }
            "description" => {
                description = expect_opt_string(src, &section, &key, attr.value)?;
            }
            "avatar" => {
                let val_span = attr.value.1;
                if let Some(path) = expect_opt_string(src, &section, &key, attr.value)? {
                    avatar = Some((path, source_span(src, val_span)));
                }
            }
            "belong_to" => {
                belong_to = expect_name_list(src, &section, &key, attr.value)?;
            }
            "calculable_props" => {
                calculable_props = expect_name_list(src, &section, &key, attr.value)?;
            }
            _ => {
                let value = expr_to_json(src, &section, &key, attr.value)?;
                extra_props.insert(key, value);
            }
        }
    }

    Ok((
        RawEntity {
            base_time: None,
            description,
            avatar,
            belong_to,
            calculable_props,
            extra_props,
        },
        base_time,
    ))
}

type LoweredEvent = (
    Vec<(String, SourceSpan)>,
    Option<String>,
    Spanned<Expr>,
    Option<Spanned<Expr>>,
);

fn lower_event(
    src: &str,
    id: &str,
    attrs: Vec<Spanned<Attr>>,
    block_span: Span,
) -> Result<LoweredEvent, DocumentError> {
    let section = format!("event `{id}`");
    let mut start = None;
    let mut end = None;
    let mut entities = Vec::new();
    let mut description = None;

    for (attr, _) in attrs {
        let key = attr.key.0;
        let key_span = attr.key.1;
        match key.as_str() {
            "start" => start = Some(attr.value),
            "end" => {
                if !matches!(attr.value.0, Expr::Null) {
                    end = Some(attr.value);
                }
            }
            "entities" => {
                entities = expect_name_list(src, &section, &key, attr.value)?;
            }
            "description" => {
                description = expect_opt_string(src, &section, &key, attr.value)?;
            }
            _ => {
                return Err(DocumentError::validation_at(
                    ValidationError::UnknownField {
                        section,
                        field: key,
                    },
                    source_span(src, key_span),
                ));
            }
        }
    }

    let start = start.ok_or_else(|| {
        DocumentError::validation_at(
            ValidationError::MissingField {
                section,
                field: "start".to_owned(),
            },
            source_span(src, block_span),
        )
    })?;

    Ok((entities, description, start, end))
}

fn expect_string(
    src: &str,
    section: &str,
    field: &str,
    expr: Spanned<Expr>,
) -> Result<String, DocumentError> {
    match expr.0 {
        Expr::Str(s) => Ok(s),
        _ => Err(DocumentError::validation_at(
            ValidationError::InvalidField {
                section: section.to_owned(),
                field: field.to_owned(),
                reason: "expected a string".to_owned(),
            },
            source_span(src, expr.1),
        )),
    }
}

fn expect_opt_string(
    src: &str,
    section: &str,
    field: &str,
    expr: Spanned<Expr>,
) -> Result<Option<String>, DocumentError> {
    match expr.0 {
        Expr::Null => Ok(None),
        Expr::Str(s) => Ok(Some(s)),
        _ => Err(DocumentError::validation_at(
            ValidationError::InvalidField {
                section: section.to_owned(),
                field: field.to_owned(),
                reason: "expected a string or null".to_owned(),
            },
            source_span(src, expr.1),
        )),
    }
}

fn expect_string_map(
    src: &str,
    section: &str,
    field: &str,
    expr: Spanned<Expr>,
) -> Result<HashMap<String, String>, DocumentError> {
    let Expr::Object(entries) = expr.0 else {
        return Err(DocumentError::validation_at(
            ValidationError::InvalidField {
                section: section.to_owned(),
                field: field.to_owned(),
                reason: "expected a map of strings".to_owned(),
            },
            source_span(src, expr.1),
        ));
    };
    let mut map = HashMap::with_capacity(entries.len());
    for (entry, _) in entries {
        let value = expect_string(src, section, &entry.key.0, entry.value)?;
        map.insert(entry.key.0, value);
    }
    Ok(map)
}

fn expect_name_list(
    src: &str,
    section: &str,
    field: &str,
    expr: Spanned<Expr>,
) -> Result<Vec<(String, SourceSpan)>, DocumentError> {
    let Expr::List(items) = expr.0 else {
        return Err(DocumentError::validation_at(
            ValidationError::InvalidField {
                section: section.to_owned(),
                field: field.to_owned(),
                reason: "expected a list of names".to_owned(),
            },
            source_span(src, expr.1),
        ));
    };
    let mut out = Vec::with_capacity(items.len());
    for (item, span) in items {
        match item {
            Expr::Ident(s) | Expr::Str(s) => out.push((s, source_span(src, span))),
            _ => {
                return Err(DocumentError::validation_at(
                    ValidationError::InvalidField {
                        section: section.to_owned(),
                        field: field.to_owned(),
                        reason: "list elements must be identifiers or strings".to_owned(),
                    },
                    source_span(src, span),
                ));
            }
        }
    }
    Ok(out)
}

fn expr_to_json(
    src: &str,
    section: &str,
    field: &str,
    expr: Spanned<Expr>,
) -> Result<JsonValue, DocumentError> {
    let (node, span) = expr;
    match node {
        Expr::Null => Ok(JsonValue::Null),
        Expr::Bool(b) => Ok(JsonValue::Bool(b)),
        Expr::Int(n) => Ok(JsonValue::Number(JsonNumber::from(n))),
        Expr::Float(f) => JsonNumber::from_f64(f)
            .map(JsonValue::Number)
            .ok_or_else(|| {
                DocumentError::validation_at(
                    ValidationError::InvalidField {
                        section: section.to_owned(),
                        field: field.to_owned(),
                        reason: "non-finite float is not supported".to_owned(),
                    },
                    source_span(src, span),
                )
            }),
        Expr::Str(s) | Expr::Ident(s) => Ok(JsonValue::String(s)),
        Expr::List(items) => items
            .into_iter()
            .map(|item| expr_to_json(src, section, field, item))
            .collect::<Result<Vec<_>, _>>()
            .map(JsonValue::Array),
        Expr::Object(entries) => {
            let mut map = JsonMap::with_capacity(entries.len());
            for (entry, _) in entries {
                let value = expr_to_json(src, section, field, entry.value)?;
                map.insert(entry.key.0, value);
            }
            Ok(JsonValue::Object(map))
        }
        Expr::Member { .. } | Expr::Binary { .. } => Err(DocumentError::validation_at(
            ValidationError::InvalidField {
                section: section.to_owned(),
                field: field.to_owned(),
                reason: "expressions are only supported in time fields".to_owned(),
            },
            source_span(src, span),
        )),
    }
}
