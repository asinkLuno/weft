//! `.weft` 编译器管线：容错 AST 降级、多诊断语义校验、时变图代数校验、Lint 检查与运行时 `WeftDoc` 构建。

use std::{
    collections::{BTreeMap, HashMap},
    path::Path,
};

use serde::Serialize;
use serde_json::{Map as JsonMap, Number as JsonNumber, Value as JsonValue};

use crate::entity::{
    Entity, EntityBuildContext, RawEntity, build_entities_runtime, validate_entities_static,
};
use crate::errors::{ByteSpan, Diagnostic, DocumentError, ValidationError};
use crate::event::{
    Event, RawEvent, build_events_collecting, check_collecting as check_events_collecting,
};
use crate::events::{Events, check_collecting as check_event_lists_collecting};
use crate::lint::{LintContext, run_lints};
use crate::parser::{
    Attr, Expr, LinkEdge, LinkEdgeKind, Span, Spanned, TopItem, WeftAst, parse_weft_recover,
    to_byte_span,
};
use crate::phase::{TimeResolver, TimeTarget};
use crate::relation::{
    Link, LinkGraph, LinkRuntimeContext, RawLink, RawLinkGraph, build_and_validate_links_runtime,
    validate_graphs_static, validate_links_static, validate_timeless_links_fallback,
};
use crate::story::{RawStory, Story, StoryContext};

/// 编译后的运行时故事文档，直接序列化为 JSON 交付前端。
#[derive(Debug, Serialize)]
pub struct WeftDoc {
    pub story: Story,
    pub graphs: BTreeMap<String, LinkGraph>,
    pub entity: HashMap<String, Entity>,
    pub links: Vec<Link>,
    pub event: BTreeMap<String, Event>,
    pub events: BTreeMap<String, Events>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calendar_components: Option<usize>,
}

/// 从 `story` 块提取的脚本依赖路径，供文件监听器跟踪 `.rn` 脚本变更。
pub struct StoryScriptRefs {
    pub date_mode: Option<String>,
    pub calculable_props: Vec<String>,
}

/// 检查选项：可控制是否执行外部 `.rn` 脚本及历法语言。
#[derive(Clone, Debug)]
pub struct CheckOptions {
    pub run_scripts: bool,
    pub lang: String,
}

impl Default for CheckOptions {
    fn default() -> Self {
        Self {
            run_scripts: true,
            lang: "zh".to_owned(),
        }
    }
}

/// 单次检查/编译的完整产出：包含全部语法、语义与 Lint 诊断。
pub struct CheckOutput {
    pub doc: Option<WeftDoc>,
    pub scripts: Option<StoryScriptRefs>,
    pub diagnostics: Vec<Diagnostic>,
}

impl CheckOutput {
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(Diagnostic::is_error)
    }
}

/// 统一检查与编译入口：收集全部语法错误、语义校验错误与 Lint 警告；无错且启用脚本时返回完整 `WeftDoc`。
pub fn check_document(src: &str, base: &Path, options: &CheckOptions) -> CheckOutput {
    let (ast, mut diagnostics) = parse_weft_recover(src);
    let Some(ast) = ast else {
        for diag in &mut diagnostics {
            diag.localize(&options.lang);
        }
        sort_diagnostics(&mut diagnostics);
        return CheckOutput {
            doc: None,
            scripts: None,
            diagnostics,
        };
    };

    let scripts = extract_script_refs(&ast);
    let doc = analyze_ast(ast, base, options, &mut diagnostics);
    for diag in &mut diagnostics {
        diag.localize(&options.lang);
    }
    sort_diagnostics(&mut diagnostics);

    let has_errors = diagnostics.iter().any(Diagnostic::is_error);
    CheckOutput {
        doc: if has_errors { None } else { doc },
        scripts,
        diagnostics,
    }
}

/// Tauri 桌面端兼容入口：单遍编译并返回脚本依赖与 `WeftDoc` 或首条错误。
pub fn compile_with_scripts(
    src: &str,
    base: &Path,
    lang: &str,
) -> (Option<StoryScriptRefs>, Result<WeftDoc, DocumentError>) {
    let out = check_document(
        src,
        base,
        &CheckOptions {
            run_scripts: true,
            lang: lang.to_owned(),
        },
    );
    if let Some(doc) = out.doc {
        return (out.scripts, Ok(doc));
    }
    let first_error = out
        .diagnostics
        .into_iter()
        .find(Diagnostic::is_error)
        .map(|diag| {
            let err = diag
                .error
                .unwrap_or_else(|| ValidationError::SyntaxError {
                    details: diag.message,
                });
            match diag.span {
                Some(span) => DocumentError::validation_at(err, span.to_source_span(src)),
                None => DocumentError::from(err),
            }
        })
        .unwrap_or_else(|| DocumentError::raw("document compilation failed"));
    (out.scripts, Err(first_error))
}

fn sort_diagnostics(diagnostics: &mut [Diagnostic]) {
    diagnostics.sort_by_key(|d| d.span.map_or(usize::MAX, |s| s.start));
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

fn analyze_ast(
    ast: WeftAst,
    base: &Path,
    options: &CheckOptions,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<WeftDoc> {
    let mut story_attrs: Option<(Vec<Spanned<Attr>>, Span)> = None;
    let mut anchor_spans: BTreeMap<String, ByteSpan> = BTreeMap::new();
    let mut time_exprs: HashMap<TimeTarget, Spanned<Expr>> = HashMap::new();
    let mut entity_blocks: BTreeMap<String, (ByteSpan, Vec<Spanned<Attr>>)> = BTreeMap::new();
    let mut event_blocks: BTreeMap<String, (ByteSpan, Vec<Spanned<Attr>>, Span)> = BTreeMap::new();
    let mut node_namespace: HashMap<String, (&'static str, ByteSpan)> = HashMap::new();
    let mut link_blocks: BTreeMap<String, (ByteSpan, ByteSpan, Vec<Spanned<Attr>>, Vec<Spanned<LinkEdge>>)> =
        BTreeMap::new();
    let mut raw_event_lists: BTreeMap<String, (ByteSpan, Vec<(String, ByteSpan)>)> =
        BTreeMap::new();

    for (item, item_span) in ast.items {
        match item {
            TopItem::Story(attrs) => {
                if let Some((_, first_span)) = &story_attrs {
                    diagnostics.push(
                        Diagnostic::from_validation(
                            ValidationError::DuplicateStory,
                            Some(to_byte_span(item_span)),
                        )
                        .with_related(to_byte_span(*first_span), "first story block defined here"),
                    );
                } else {
                    story_attrs = Some((attrs, item_span));
                }
            }
            TopItem::TimeAnchor { name, expr } => {
                let name_span = to_byte_span(name.1);
                if let Some(&first_span) = anchor_spans.get(&name.0) {
                    diagnostics.push(
                        Diagnostic::from_validation(
                            ValidationError::DuplicateTimeAnchor { name: name.0 },
                            Some(name_span),
                        )
                        .with_related(first_span, "first defined here"),
                    );
                } else {
                    anchor_spans.insert(name.0.clone(), name_span);
                    time_exprs.insert(TimeTarget::Anchor(name.0), expr);
                }
            }
            TopItem::Entity { name, attrs } => {
                let name_span = to_byte_span(name.1);
                if let Some((first_span, _)) = entity_blocks.get(&name.0) {
                    diagnostics.push(
                        Diagnostic::from_validation(
                            ValidationError::DuplicateEntity { entity: name.0 },
                            Some(name_span),
                        )
                        .with_related(*first_span, "first defined here"),
                    );
                } else if let Some(&(existing_kind, first_span)) = node_namespace.get(&name.0) {
                    diagnostics.push(
                        Diagnostic::from_validation(
                            ValidationError::DuplicateSymbol {
                                name: name.0,
                                existing_kind: existing_kind.to_owned(),
                                new_kind: "entity".to_owned(),
                            },
                            Some(name_span),
                        )
                        .with_related(first_span, "first declared here"),
                    );
                } else {
                    node_namespace.insert(name.0.clone(), ("entity", name_span));
                    entity_blocks.insert(name.0, (name_span, attrs));
                }
            }
            TopItem::Event { name, attrs } => {
                let name_span = to_byte_span(name.1);
                if let Some((first_span, _, _)) = event_blocks.get(&name.0) {
                    diagnostics.push(
                        Diagnostic::from_validation(
                            ValidationError::DuplicateEvent { event: name.0 },
                            Some(name_span),
                        )
                        .with_related(*first_span, "first defined here"),
                    );
                } else if let Some(&(existing_kind, first_span)) = node_namespace.get(&name.0) {
                    diagnostics.push(
                        Diagnostic::from_validation(
                            ValidationError::DuplicateSymbol {
                                name: name.0,
                                existing_kind: existing_kind.to_owned(),
                                new_kind: "event".to_owned(),
                            },
                            Some(name_span),
                        )
                        .with_related(first_span, "first declared here"),
                    );
                } else {
                    node_namespace.insert(name.0.clone(), ("event", name_span));
                    event_blocks.insert(name.0, (name_span, attrs, item_span));
                }
            }
            TopItem::Link { name, attrs, edges } => {
                let name_span = to_byte_span(name.1);
                if let Some((first_span, _, _, _)) = link_blocks.get(&name.0) {
                    diagnostics.push(
                        Diagnostic::from_validation(
                            ValidationError::DuplicateLinkGraph { graph: name.0 },
                            Some(name_span),
                        )
                        .with_related(*first_span, "first defined here"),
                    );
                } else {
                    link_blocks.insert(name.0, (name_span, to_byte_span(item_span), attrs, edges));
                }
            }
            TopItem::Events { name, events } => {
                let name_span = to_byte_span(name.1);
                if let Some((first_span, _)) = raw_event_lists.get(&name.0) {
                    diagnostics.push(
                        Diagnostic::from_validation(
                            ValidationError::DuplicateEvents { events: name.0 },
                            Some(name_span),
                        )
                        .with_related(*first_span, "first defined here"),
                    );
                } else {
                    let refs = events
                        .into_iter()
                        .map(|(ev, sp)| (ev, to_byte_span(sp)))
                        .collect();
                    raw_event_lists.insert(name.0, (name_span, refs));
                }
            }
        }
    }

    let raw_story = match story_attrs {
        Some((attrs, span)) => lower_story(attrs, span, diagnostics),
        None => {
            diagnostics.push(Diagnostic::from_validation(
                ValidationError::MissingStory,
                None,
            ));
            None
        }
    };

    let mut raw_graphs: BTreeMap<String, RawLinkGraph> = BTreeMap::new();
    let mut raw_links: Vec<RawLink> = Vec::new();
    let mut next_stmt_id = 0usize;

    for (graph_name, (name_span, block_span, attrs, edges)) in link_blocks {
        let edge_stmt_count = edges.len();
        let raw_graph = lower_link_graph(
            &graph_name,
            name_span,
            block_span,
            attrs,
            edge_stmt_count,
            diagnostics,
        );
        raw_graphs.insert(graph_name.clone(), raw_graph);

        for (edge, edge_span) in edges {
            let stmt_id = next_stmt_id;
            next_stmt_id += 1;
            let edge_byte_span = to_byte_span(edge_span);
            let directed = edge.is_directed();
            let label = edge.label.map(|(s, sp)| (s, to_byte_span(sp)));

            let (description, start_expr, end_expr) = match edge.attrs {
                Some((edge_attrs, _)) => lower_link_attrs(&graph_name, edge_attrs, diagnostics),
                None => (None, None, None),
            };
            let start_attr_span = start_expr.as_ref().map(|(_, sp)| to_byte_span(*sp));
            let end_attr_span = end_expr.as_ref().map(|(_, sp)| to_byte_span(*sp));
            if let Some(expr) = start_expr {
                time_exprs.insert(TimeTarget::LinkStart(stmt_id), expr);
            }
            if let Some(expr) = end_expr {
                time_exprs.insert(TimeTarget::LinkEnd(stmt_id), expr);
            }

            match edge.kind {
                LinkEdgeKind::Directed { left, right } | LinkEdgeKind::Symmetric { left, right } => {
                    let left_names = left.names();
                    let right_names = right.names();
                    if left_names.is_empty() {
                        diagnostics.push(Diagnostic::from_validation(
                            ValidationError::EmptyLinkEndpoint {
                                graph: graph_name.clone(),
                            },
                            Some(to_byte_span(left.span())),
                        ));
                        continue;
                    }
                    if right_names.is_empty() {
                        diagnostics.push(Diagnostic::from_validation(
                            ValidationError::EmptyLinkEndpoint {
                                graph: graph_name.clone(),
                            },
                            Some(to_byte_span(right.span())),
                        ));
                        continue;
                    }
                    for (l_name, l_span) in left_names {
                        for (r_name, r_span) in right_names {
                            raw_links.push(RawLink {
                                stmt_id,
                                graph: graph_name.clone(),
                                from: (l_name.clone(), to_byte_span(*l_span)),
                                to: (r_name.clone(), to_byte_span(*r_span)),
                                directed,
                                label: label.clone(),
                                start_attr_span,
                                end_attr_span,
                                start_time: None,
                                end_time: None,
                                description: description.clone(),
                                span: edge_byte_span,
                            });
                        }
                    }
                }
                LinkEdgeKind::Clique { members, list_span } => {
                    if members.len() < 2 {
                        diagnostics.push(Diagnostic::from_validation(
                            ValidationError::InvalidClique {
                                graph: graph_name.clone(),
                                count: members.len(),
                            },
                            Some(to_byte_span(list_span)),
                        ));
                        continue;
                    }
                    for i in 0..members.len() {
                        for j in (i + 1)..members.len() {
                            let (a_name, a_span) = &members[i];
                            let (b_name, b_span) = &members[j];
                            raw_links.push(RawLink {
                                stmt_id,
                                graph: graph_name.clone(),
                                from: (a_name.clone(), to_byte_span(*a_span)),
                                to: (b_name.clone(), to_byte_span(*b_span)),
                                directed: false,
                                label: label.clone(),
                                start_attr_span,
                                end_attr_span,
                                start_time: None,
                                end_time: None,
                                description: description.clone(),
                                span: edge_byte_span,
                            });
                        }
                    }
                }
            }
        }
    }

    // 收集实体/事件字段并把所有时间表达式汇入同一张 `time_exprs` 表，交由统一 TimeResolver 展开。
    let mut raw_entities: BTreeMap<String, RawEntity> = BTreeMap::new();
    for (id, (name_span, attrs)) in entity_blocks {
        let (raw, base_time_expr) = lower_entity(&id, name_span, attrs, diagnostics);
        if let Some(expr) = base_time_expr {
            time_exprs.insert(TimeTarget::EntityBaseTime(id.clone()), expr);
        }
        raw_entities.insert(id, raw);
    }

    let mut pending_events: BTreeMap<
        String,
        (ByteSpan, Vec<(String, ByteSpan)>, Option<String>, bool),
    > = BTreeMap::new();
    for (id, (name_span, attrs, ev_span)) in event_blocks {
        let (entities, description, start_expr, end_expr) =
            lower_event(&id, attrs, ev_span, diagnostics);
        let has_start = start_expr.is_some();
        if let Some(start_expr) = start_expr {
            time_exprs.insert(TimeTarget::EventStart(id.clone()), start_expr);
        }
        if let Some(end_expr) = end_expr {
            time_exprs.insert(TimeTarget::EventEnd(id.clone()), end_expr);
        }
        pending_events.insert(id, (name_span, entities, description, has_start));
    }

    let mut resolver = TimeResolver::new(&time_exprs);

    // 校验所有独立声明的 time 锚点（即使未被事件或边引用也提前捕获错误）。
    for name in anchor_spans.keys() {
        let target = TimeTarget::Anchor(name.clone());
        let span = time_exprs[&target].1;
        resolver.resolve_target(&target, span, diagnostics);
    }

    for (id, raw) in &mut raw_entities {
        let target = TimeTarget::EntityBaseTime(id.clone());
        if let Some(expr) = time_exprs.get(&target)
            && let Some(phase) = resolver.resolve_target(&target, expr.1, diagnostics)
        {
            raw.base_time = Some((phase, to_byte_span(expr.1)));
        }
    }

    let mut raw_events: BTreeMap<String, RawEvent> = BTreeMap::new();
    for (id, (name_span, entities, description, has_start)) in pending_events {
        let start_time = if has_start {
            let start_target = TimeTarget::EventStart(id.clone());
            let start_span = time_exprs[&start_target].1;
            resolver
                .resolve_target(&start_target, start_span, diagnostics)
                .map(|phase| (phase, to_byte_span(start_span)))
        } else {
            None
        };

        let end_target = TimeTarget::EventEnd(id.clone());
        let end_time = time_exprs.get(&end_target).and_then(|expr| {
            resolver
                .resolve_target(&end_target, expr.1, diagnostics)
                .map(|phase| (phase, to_byte_span(expr.1)))
        });

        raw_events.insert(
            id,
            RawEvent {
                name_span,
                start_time,
                end_time,
                entities,
                description,
            },
        );
    }

    let mut resolved_stmt_times: HashMap<usize, (Option<(crate::phase::Phase, ByteSpan)>, Option<(crate::phase::Phase, ByteSpan)>)> =
        HashMap::new();
    for link in &mut raw_links {
        let (st, et) = resolved_stmt_times
            .entry(link.stmt_id)
            .or_insert_with(|| {
                let start_target = TimeTarget::LinkStart(link.stmt_id);
                let s = time_exprs.get(&start_target).and_then(|expr| {
                    resolver
                        .resolve_target(&start_target, expr.1, diagnostics)
                        .map(|phase| (phase, to_byte_span(expr.1)))
                });
                let end_target = TimeTarget::LinkEnd(link.stmt_id);
                let e = time_exprs.get(&end_target).and_then(|expr| {
                    resolver
                        .resolve_target(&end_target, expr.1, diagnostics)
                        .map(|phase| (phase, to_byte_span(expr.1)))
                });
                (s, e)
            })
            .clone();
        link.start_time = st;
        link.end_time = et;
    }

    let used_anchors = resolver.into_used_anchors();

    // 静态图配置、引用与结构校验（不依赖外部脚本，总是完整执行）。
    let mut valid_graphs = validate_graphs_static(&raw_graphs, diagnostics);
    validate_entities_static(&raw_entities, base, options.run_scripts, diagnostics);
    let link_static_valid = validate_links_static(
        &raw_links,
        &mut raw_graphs,
        &mut valid_graphs,
        &raw_entities,
        &raw_events,
        diagnostics,
    );
    check_events_collecting(&raw_events, &raw_entities, diagnostics);
    check_event_lists_collecting(&raw_event_lists, &raw_events, diagnostics);

    // 静态 Lint 告警（unused / self-link / empty-block）。
    run_lints(
        &LintContext {
            lang: &options.lang,
            anchors: &anchor_spans,
            used_anchors: &used_anchors,
            graphs: &raw_graphs,
            entities: &raw_entities,
            links: &raw_links,
            events: &raw_events,
            event_lists: &raw_event_lists,
        },
        diagnostics,
    );

    if !options.run_scripts {
        validate_timeless_links_fallback(
            &raw_links,
            &link_static_valid,
            &raw_graphs,
            &valid_graphs,
            &options.lang,
            diagnostics,
        );
        return None;
    }

    // 运行时脚本阶段：历法加载、可计算属性求值、时变图代数校验与时间归一化。
    let Some(raw_story) = raw_story else {
        validate_timeless_links_fallback(
            &raw_links,
            &link_static_valid,
            &raw_graphs,
            &valid_graphs,
            &options.lang,
            diagnostics,
        );
        return None;
    };
    let Some(story) = Story::build_collecting(
        raw_story,
        StoryContext {
            base,
            lang: &options.lang,
        },
        diagnostics,
    ) else {
        validate_timeless_links_fallback(
            &raw_links,
            &link_static_valid,
            &raw_graphs,
            &valid_graphs,
            &options.lang,
            diagnostics,
        );
        return None;
    };

    let (entity, entity_base_times) = build_entities_runtime(
        raw_entities,
        EntityBuildContext {
            base,
            calendar: &story.calendar,
            calculable_props: &story.calculable_prop_runtime,
        },
        diagnostics,
    );
    let event =
        build_events_collecting(raw_events, &entity_base_times, &story.calendar, diagnostics);
    let links = build_and_validate_links_runtime(
        &raw_links,
        &link_static_valid,
        &LinkRuntimeContext {
            graphs: &raw_graphs,
            valid_graphs: &valid_graphs,
            entities: &entity,
            entity_base_times: &entity_base_times,
            events: &event,
            calendar: &story.calendar,
            lang: &options.lang,
        },
        diagnostics,
    );
    let events: BTreeMap<String, Events> = raw_event_lists
        .into_iter()
        .map(|(name, (_, refs))| (name, refs.into_iter().map(|(id, _)| id).collect()))
        .collect();
    let graphs: BTreeMap<String, LinkGraph> = raw_graphs
        .into_iter()
        .map(|(name, raw)| (name, raw.spec))
        .collect();
    let calendar_components = Some(story.calendar.component_count());

    Some(WeftDoc {
        story,
        graphs,
        entity,
        links,
        event,
        events,
        calendar_components,
    })
}

// ---- AST Lowering（收集全部字段诊断） ----

fn track_duplicate_attr(
    seen: &mut HashMap<String, ByteSpan>,
    section: &str,
    key: &str,
    key_span: ByteSpan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(&first_span) = seen.get(key) {
        diagnostics.push(
            Diagnostic::warning(
                "duplicate-attr",
                format!("duplicate attribute `{key}` in {section}; earlier value is overwritten"),
                Some(key_span),
            )
            .with_related(first_span, "previously set here"),
        );
    } else {
        seen.insert(key.to_owned(), key_span);
    }
}

fn lower_story(
    attrs: Vec<Spanned<Attr>>,
    block_span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<RawStory> {
    let section = "story";
    let mut seen_keys = HashMap::new();
    let mut title = None;
    let mut description = None;
    let mut author = None;
    let mut date_mode = None;
    let mut calculable_props = HashMap::new();

    for (attr, _) in attrs {
        let key = attr.key.0;
        let key_span = to_byte_span(attr.key.1);
        track_duplicate_attr(&mut seen_keys, section, &key, key_span, diagnostics);
        match key.as_str() {
            "title" => {
                if let Some(s) = expect_string(section, &key, attr.value, diagnostics) {
                    title = Some(s);
                }
            }
            "description" => {
                if let Some(val) = expect_opt_string(section, &key, attr.value, diagnostics) {
                    description = val;
                }
            }
            "author" => {
                if let Some(val) = expect_opt_string(section, &key, attr.value, diagnostics) {
                    author = val;
                }
            }
            "date_mode" => {
                let val_span = to_byte_span(attr.value.1);
                if let Some(val) = expect_opt_string(section, &key, attr.value, diagnostics) {
                    date_mode = val.map(|path| (path, val_span));
                }
            }
            "calculable_props" => {
                if let Some(map) = expect_string_map(section, &key, attr.value, diagnostics) {
                    calculable_props = map;
                }
            }
            _ => {
                diagnostics.push(Diagnostic::from_validation(
                    ValidationError::UnknownField {
                        section: section.to_owned(),
                        field: key,
                    },
                    Some(key_span),
                ));
            }
        }
    }

    let block_span = to_byte_span(block_span);
    let title = match title {
        Some(t) => t,
        None => {
            diagnostics.push(Diagnostic::from_validation(
                ValidationError::MissingField {
                    section: section.to_owned(),
                    field: "title".to_owned(),
                },
                Some(block_span),
            ));
            String::new()
        }
    };

    Some(RawStory {
        title,
        description,
        author,
        date_mode,
        calculable_props,
        block_span,
    })
}

fn lower_link_graph(
    name: &str,
    name_span: ByteSpan,
    block_span: ByteSpan,
    attrs: Vec<Spanned<Attr>>,
    edge_stmt_count: usize,
    diagnostics: &mut Vec<Diagnostic>,
) -> RawLinkGraph {
    let section = format!("link `{name}`");
    let mut seen_keys = HashMap::new();
    let mut spec = LinkGraph::default();
    let mut attr_spans = HashMap::new();

    for (attr, attr_span) in attrs {
        let key = attr.key.0;
        let key_span = to_byte_span(attr.key.1);
        track_duplicate_attr(&mut seen_keys, &section, &key, key_span, diagnostics);
        match key.as_str() {
            "acyclic" => {
                if let Some(v) = expect_bool(&section, &key, attr.value, diagnostics) {
                    spec.acyclic = v;
                    attr_spans.insert(key, to_byte_span(attr_span));
                }
            }
            "asymmetric" => {
                if let Some(v) = expect_bool(&section, &key, attr.value, diagnostics) {
                    spec.asymmetric = v;
                    attr_spans.insert(key, to_byte_span(attr_span));
                }
            }
            "intransitive" => {
                if let Some(v) = expect_bool(&section, &key, attr.value, diagnostics) {
                    spec.intransitive = v;
                    attr_spans.insert(key, to_byte_span(attr_span));
                }
            }
            "functional" => {
                if let Some(v) = expect_bool(&section, &key, attr.value, diagnostics) {
                    spec.functional = v;
                    attr_spans.insert(key, to_byte_span(attr_span));
                }
            }
            "inverse_functional" => {
                if let Some(v) = expect_bool(&section, &key, attr.value, diagnostics) {
                    spec.inverse_functional = v;
                    attr_spans.insert(key, to_byte_span(attr_span));
                }
            }
            "symmetric" => {
                if let Some(v) = expect_bool(&section, &key, attr.value, diagnostics) {
                    spec.symmetric = v;
                    attr_spans.insert(key, to_byte_span(attr_span));
                }
            }
            "transitive" => {
                if let Some(v) = expect_bool(&section, &key, attr.value, diagnostics) {
                    spec.transitive = v;
                    attr_spans.insert(key, to_byte_span(attr_span));
                }
            }
            "equivalence" => {
                if let Some(v) = expect_bool(&section, &key, attr.value, diagnostics) {
                    spec.equivalence = v;
                    attr_spans.insert(key, to_byte_span(attr_span));
                }
            }
            "chronological" => {
                if let Some(v) = expect_bool(&section, &key, attr.value, diagnostics) {
                    spec.chronological = v;
                    attr_spans.insert(key, to_byte_span(attr_span));
                }
            }
            "description" => {
                if let Some(val) = expect_opt_string(&section, &key, attr.value, diagnostics) {
                    spec.description = val;
                }
            }
            _ => {
                diagnostics.push(Diagnostic::from_validation(
                    ValidationError::UnknownField {
                        section: section.clone(),
                        field: key,
                    },
                    Some(key_span),
                ));
            }
        }
    }

    RawLinkGraph {
        name_span,
        block_span,
        spec,
        attr_spans,
        edge_stmt_count,
    }
}

fn lower_entity(
    id: &str,
    name_span: ByteSpan,
    attrs: Vec<Spanned<Attr>>,
    diagnostics: &mut Vec<Diagnostic>,
) -> (RawEntity, Option<Spanned<Expr>>) {
    let section = format!("entity `{id}`");
    let mut seen_keys = HashMap::new();
    let mut base_time = None;
    let mut description = None;
    let mut avatar = None;
    let mut calculable_props = Vec::new();
    let mut extra_props = HashMap::new();

    for (attr, _) in attrs {
        let key = attr.key.0;
        let key_span = to_byte_span(attr.key.1);
        track_duplicate_attr(&mut seen_keys, &section, &key, key_span, diagnostics);
        match key.as_str() {
            "base_time" => {
                if !matches!(attr.value.0, Expr::Null) {
                    base_time = Some(attr.value);
                } else {
                    base_time = None;
                }
            }
            "description" => {
                if let Some(val) = expect_opt_string(&section, &key, attr.value, diagnostics) {
                    description = val;
                }
            }
            "avatar" => {
                let val_span = to_byte_span(attr.value.1);
                if let Some(val) = expect_opt_string(&section, &key, attr.value, diagnostics) {
                    avatar = val.map(|path| (path, val_span));
                }
            }
            "calculable_props" => {
                if let Some(list) = expect_name_list(&section, &key, attr.value, diagnostics) {
                    calculable_props = list;
                }
            }
            _ => {
                if let Some(value) = expr_to_json(&section, &key, attr.value, diagnostics) {
                    extra_props.insert(key, value);
                }
            }
        }
    }

    (
        RawEntity {
            name_span,
            base_time: None,
            description,
            avatar,
            calculable_props,
            extra_props,
        },
        base_time,
    )
}

type LoweredLinkAttrs = (
    Option<String>,
    Option<Spanned<Expr>>,
    Option<Spanned<Expr>>,
);

fn lower_link_attrs(
    graph: &str,
    attrs: Vec<Spanned<Attr>>,
    diagnostics: &mut Vec<Diagnostic>,
) -> LoweredLinkAttrs {
    let section = format!("link `{graph}` edge");
    let mut seen_keys = HashMap::new();
    let mut start = None;
    let mut end = None;
    let mut description = None;

    for (attr, _) in attrs {
        let key = attr.key.0;
        let key_span = to_byte_span(attr.key.1);
        track_duplicate_attr(&mut seen_keys, &section, &key, key_span, diagnostics);
        match key.as_str() {
            "start" => {
                if !matches!(attr.value.0, Expr::Null) {
                    start = Some(attr.value);
                } else {
                    start = None;
                }
            }
            "end" => {
                if !matches!(attr.value.0, Expr::Null) {
                    end = Some(attr.value);
                } else {
                    end = None;
                }
            }
            "description" => {
                if let Some(val) = expect_opt_string(&section, &key, attr.value, diagnostics) {
                    description = val;
                }
            }
            _ => {
                diagnostics.push(Diagnostic::from_validation(
                    ValidationError::UnknownField {
                        section: section.clone(),
                        field: key,
                    },
                    Some(key_span),
                ));
            }
        }
    }

    (description, start, end)
}

type LoweredEvent = (
    Vec<(String, ByteSpan)>,
    Option<String>,
    Option<Spanned<Expr>>,
    Option<Spanned<Expr>>,
);

fn lower_event(
    id: &str,
    attrs: Vec<Spanned<Attr>>,
    block_span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) -> LoweredEvent {
    let section = format!("event `{id}`");
    let mut seen_keys = HashMap::new();
    let mut start = None;
    let mut end = None;
    let mut entities = Vec::new();
    let mut description = None;

    for (attr, _) in attrs {
        let key = attr.key.0;
        let key_span = to_byte_span(attr.key.1);
        track_duplicate_attr(&mut seen_keys, &section, &key, key_span, diagnostics);
        match key.as_str() {
            "start" => start = Some(attr.value),
            "end" => {
                if !matches!(attr.value.0, Expr::Null) {
                    end = Some(attr.value);
                } else {
                    end = None;
                }
            }
            "entities" => {
                if let Some(list) = expect_name_list(&section, &key, attr.value, diagnostics) {
                    entities = list;
                }
            }
            "description" => {
                if let Some(val) = expect_opt_string(&section, &key, attr.value, diagnostics) {
                    description = val;
                }
            }
            _ => {
                diagnostics.push(Diagnostic::from_validation(
                    ValidationError::UnknownField {
                        section: section.clone(),
                        field: key,
                    },
                    Some(key_span),
                ));
            }
        }
    }

    if start.is_none() {
        diagnostics.push(Diagnostic::from_validation(
            ValidationError::MissingField {
                section,
                field: "start".to_owned(),
            },
            Some(to_byte_span(block_span)),
        ));
    }

    (entities, description, start, end)
}

fn expect_bool(
    section: &str,
    field: &str,
    expr: Spanned<Expr>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<bool> {
    match expr.0 {
        Expr::Bool(b) => Some(b),
        _ => {
            diagnostics.push(Diagnostic::from_validation(
                ValidationError::InvalidField {
                    section: section.to_owned(),
                    field: field.to_owned(),
                    reason: "expected a boolean (`true` or `false`)".to_owned(),
                },
                Some(to_byte_span(expr.1)),
            ));
            None
        }
    }
}

fn expect_string(
    section: &str,
    field: &str,
    expr: Spanned<Expr>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<String> {
    match expr.0 {
        Expr::Str(s) => Some(s),
        _ => {
            diagnostics.push(Diagnostic::from_validation(
                ValidationError::InvalidField {
                    section: section.to_owned(),
                    field: field.to_owned(),
                    reason: "expected a string".to_owned(),
                },
                Some(to_byte_span(expr.1)),
            ));
            None
        }
    }
}

fn expect_opt_string(
    section: &str,
    field: &str,
    expr: Spanned<Expr>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Option<String>> {
    match expr.0 {
        Expr::Null => Some(None),
        Expr::Str(s) => Some(Some(s)),
        _ => {
            diagnostics.push(Diagnostic::from_validation(
                ValidationError::InvalidField {
                    section: section.to_owned(),
                    field: field.to_owned(),
                    reason: "expected a string or null".to_owned(),
                },
                Some(to_byte_span(expr.1)),
            ));
            None
        }
    }
}

fn expect_string_map(
    section: &str,
    field: &str,
    expr: Spanned<Expr>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<HashMap<String, (String, ByteSpan)>> {
    let Expr::Object(entries) = expr.0 else {
        diagnostics.push(Diagnostic::from_validation(
            ValidationError::InvalidField {
                section: section.to_owned(),
                field: field.to_owned(),
                reason: "expected a map of strings".to_owned(),
            },
            Some(to_byte_span(expr.1)),
        ));
        return None;
    };
    let mut map = HashMap::with_capacity(entries.len());
    for (entry, _) in entries {
        let val_span = to_byte_span(entry.value.1);
        if let Some(value) = expect_string(section, &entry.key.0, entry.value, diagnostics) {
            map.insert(entry.key.0, (value, val_span));
        }
    }
    Some(map)
}

fn expect_name_list(
    section: &str,
    field: &str,
    expr: Spanned<Expr>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Vec<(String, ByteSpan)>> {
    let Expr::List(items) = expr.0 else {
        diagnostics.push(Diagnostic::from_validation(
            ValidationError::InvalidField {
                section: section.to_owned(),
                field: field.to_owned(),
                reason: "expected a list of names".to_owned(),
            },
            Some(to_byte_span(expr.1)),
        ));
        return None;
    };
    let mut out = Vec::with_capacity(items.len());
    for (item, span) in items {
        match item {
            Expr::Ident(s) | Expr::Str(s) => out.push((s, to_byte_span(span))),
            _ => {
                diagnostics.push(Diagnostic::from_validation(
                    ValidationError::InvalidField {
                        section: section.to_owned(),
                        field: field.to_owned(),
                        reason: "list elements must be identifiers or strings".to_owned(),
                    },
                    Some(to_byte_span(span)),
                ));
            }
        }
    }
    Some(out)
}

fn expr_to_json(
    section: &str,
    field: &str,
    expr: Spanned<Expr>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<JsonValue> {
    let (node, span) = expr;
    match node {
        Expr::Null => Some(JsonValue::Null),
        Expr::Bool(b) => Some(JsonValue::Bool(b)),
        Expr::Int(n) => Some(JsonValue::Number(JsonNumber::from(n))),
        Expr::Float(f) => match JsonNumber::from_f64(f) {
            Some(num) => Some(JsonValue::Number(num)),
            None => {
                diagnostics.push(Diagnostic::from_validation(
                    ValidationError::InvalidField {
                        section: section.to_owned(),
                        field: field.to_owned(),
                        reason: "non-finite float is not supported".to_owned(),
                    },
                    Some(to_byte_span(span)),
                ));
                None
            }
        },
        Expr::Str(s) | Expr::Ident(s) => Some(JsonValue::String(s)),
        Expr::List(items) => {
            let mut arr = Vec::with_capacity(items.len());
            for item in items {
                if let Some(val) = expr_to_json(section, field, item, diagnostics) {
                    arr.push(val);
                }
            }
            Some(JsonValue::Array(arr))
        }
        Expr::Object(entries) => {
            let mut map = JsonMap::with_capacity(entries.len());
            for (entry, _) in entries {
                if let Some(value) = expr_to_json(section, field, entry.value, diagnostics) {
                    map.insert(entry.key.0, value);
                }
            }
            Some(JsonValue::Object(map))
        }
        Expr::Member { .. } | Expr::Binary { .. } => {
            diagnostics.push(Diagnostic::from_validation(
                ValidationError::InvalidField {
                    section: section.to_owned(),
                    field: field.to_owned(),
                    reason: "expressions are only supported in time fields".to_owned(),
                },
                Some(to_byte_span(span)),
            ));
            None
        }
    }
}
