//! 文档符号表与全量引用索引：支撑跳转定义、查找引用、全局重命名、文档大纲与 Hover。

use std::collections::BTreeMap;

use weft_core::errors::ByteSpan;
use weft_core::parser::{
    Expr, LinkEdgeKind, LinkEndpoint, Span, Spanned, TopItem, WeftAst, parse_weft_recover,
};

use crate::scanner::{TokKind, tokenize};
use crate::schema::BlockKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SymbolKind {
    Entity,
    Event,
    Events,
    TimeAnchor,
    LinkGraph,
    CalculableProp,
}

impl SymbolKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Entity => "entity",
            Self::Event => "event",
            Self::Events => "events",
            Self::TimeAnchor => "time",
            Self::LinkGraph => "link",
            Self::CalculableProp => "calculable_prop",
        }
    }
}

#[derive(Clone, Debug)]
pub struct SymbolDecl {
    pub kind: SymbolKind,
    pub name: String,
    pub name_span: ByteSpan,
    pub full_span: ByteSpan,
    pub description: Option<String>,
    pub graph_props: Vec<&'static str>,
    pub entities: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Occurrence {
    pub kind: SymbolKind,
    pub name: String,
    pub span: ByteSpan,
    pub is_definition: bool,
}

#[derive(Clone, Debug)]
pub struct AttrKeySite {
    pub block: BlockKind,
    pub key: String,
    pub span: ByteSpan,
}

#[derive(Clone, Debug)]
pub struct LinkEdgeSite {
    pub graph: String,
    pub summary: String,
    pub sample_from: Option<String>,
    pub sample_to: Option<String>,
    pub directed: bool,
    pub label: Option<String>,
    pub span: ByteSpan,
}

#[derive(Clone, Debug)]
pub struct TopDeclEntry {
    pub kind: &'static str,
    pub name: String,
    pub detail: Option<String>,
    pub name_span: ByteSpan,
    pub full_span: ByteSpan,
}

#[derive(Clone, Debug, Default)]
pub struct DocumentIndex {
    pub declarations: BTreeMap<(SymbolKind, String), SymbolDecl>,
    pub occurrences: Vec<Occurrence>,
    pub attr_keys: Vec<AttrKeySite>,
    pub link_edges: Vec<LinkEdgeSite>,
    pub top_decls: Vec<TopDeclEntry>,
}

fn to_byte_span(span: Span) -> ByteSpan {
    ByteSpan::new(span.start, span.end)
}

const GRAPH_BOOL_KEYS: &[&str] = &[
    "acyclic",
    "asymmetric",
    "intransitive",
    "functional",
    "inverse_functional",
    "symmetric",
    "transitive",
    "equivalence",
    "chronological",
];

impl DocumentIndex {
    pub fn build(src: &str) -> Self {
        let (ast, _) = parse_weft_recover(src);
        let Some(ast) = ast else {
            return Self::build_fallback_from_tokens(src);
        };
        let mut idx = Self::build_from_ast(&ast);
        if idx.top_decls.is_empty() {
            let fallback = Self::build_fallback_from_tokens(src);
            if !fallback.top_decls.is_empty() {
                idx = fallback;
            }
        }
        idx
    }

    fn build_from_ast(ast: &WeftAst) -> Self {
        let mut idx = Self::default();

        // 第一遍：收集所有顶层声明与 story.calculable_props
        for (item, item_span) in &ast.items {
            let full_span = to_byte_span(*item_span);
            match item {
                TopItem::Story(attrs) => {
                    let mut title = None;
                    for (attr, _) in attrs {
                        let key_span = to_byte_span(attr.key.1);
                        idx.attr_keys.push(AttrKeySite {
                            block: BlockKind::Story,
                            key: attr.key.0.clone(),
                            span: key_span,
                        });
                        if attr.key.0 == "title"
                            && let Expr::Str(s) = &attr.value.0
                        {
                            title = Some(s.clone());
                        }
                        if attr.key.0 == "calculable_props"
                            && let Expr::Object(entries) = &attr.value.0
                        {
                            for (entry, entry_span) in entries {
                                let name_span = to_byte_span(entry.key.1);
                                idx.register_decl(SymbolDecl {
                                    kind: SymbolKind::CalculableProp,
                                    name: entry.key.0.clone(),
                                    name_span,
                                    full_span: to_byte_span(*entry_span),
                                    description: match &entry.value.0 {
                                        Expr::Str(p) => Some(p.clone()),
                                        _ => None,
                                    },
                                    graph_props: Vec::new(),
                                    entities: Vec::new(),
                                });
                            }
                        }
                    }
                    idx.top_decls.push(TopDeclEntry {
                        kind: "story",
                        name: title.clone().unwrap_or_else(|| "story".to_owned()),
                        detail: title,
                        name_span: ByteSpan::new(
                            full_span.start,
                            (full_span.start + 5).min(full_span.end),
                        ),
                        full_span,
                    });
                }
                TopItem::TimeAnchor { name, .. } => {
                    let name_span = to_byte_span(name.1);
                    idx.register_decl(SymbolDecl {
                        kind: SymbolKind::TimeAnchor,
                        name: name.0.clone(),
                        name_span,
                        full_span,
                        description: None,
                        graph_props: Vec::new(),
                        entities: Vec::new(),
                    });
                    idx.top_decls.push(TopDeclEntry {
                        kind: "time",
                        name: name.0.clone(),
                        detail: None,
                        name_span,
                        full_span,
                    });
                }
                TopItem::Entity { name, attrs } => {
                    let name_span = to_byte_span(name.1);
                    let mut description = None;
                    for (attr, _) in attrs {
                        idx.attr_keys.push(AttrKeySite {
                            block: BlockKind::Entity,
                            key: attr.key.0.clone(),
                            span: to_byte_span(attr.key.1),
                        });
                        if attr.key.0 == "description"
                            && let Expr::Str(s) = &attr.value.0
                        {
                            description = Some(s.clone());
                        }
                    }
                    idx.register_decl(SymbolDecl {
                        kind: SymbolKind::Entity,
                        name: name.0.clone(),
                        name_span,
                        full_span,
                        description: description.clone(),
                        graph_props: Vec::new(),
                        entities: Vec::new(),
                    });
                    idx.top_decls.push(TopDeclEntry {
                        kind: "entity",
                        name: name.0.clone(),
                        detail: description,
                        name_span,
                        full_span,
                    });
                }
                TopItem::Link { name, attrs, .. } => {
                    let name_span = to_byte_span(name.1);
                    let mut description = None;
                    let mut enabled_flags: BTreeMap<&'static str, bool> = BTreeMap::new();
                    for (attr, _) in attrs {
                        idx.attr_keys.push(AttrKeySite {
                            block: BlockKind::Link,
                            key: attr.key.0.clone(),
                            span: to_byte_span(attr.key.1),
                        });
                        if attr.key.0 == "description"
                            && let Expr::Str(s) = &attr.value.0
                        {
                            description = Some(s.clone());
                        } else if let Some(&prop_key) = GRAPH_BOOL_KEYS
                            .iter()
                            .find(|&&k| k == attr.key.0.as_str())
                            && let Expr::Bool(v) = attr.value.0
                        {
                            enabled_flags.insert(prop_key, v);
                        }
                    }
                    let graph_props: Vec<&'static str> = GRAPH_BOOL_KEYS
                        .iter()
                        .copied()
                        .filter(|k| enabled_flags.get(k).copied().unwrap_or(false))
                        .collect();
                    let detail = if !graph_props.is_empty() {
                        Some(graph_props.join(", "))
                    } else {
                        description.clone()
                    };
                    idx.register_decl(SymbolDecl {
                        kind: SymbolKind::LinkGraph,
                        name: name.0.clone(),
                        name_span,
                        full_span,
                        description,
                        graph_props,
                        entities: Vec::new(),
                    });
                    idx.top_decls.push(TopDeclEntry {
                        kind: "link",
                        name: name.0.clone(),
                        detail,
                        name_span,
                        full_span,
                    });
                }
                TopItem::Event { name, attrs } => {
                    let name_span = to_byte_span(name.1);
                    let mut description = None;
                    let mut entities = Vec::new();
                    for (attr, _) in attrs {
                        idx.attr_keys.push(AttrKeySite {
                            block: BlockKind::Event,
                            key: attr.key.0.clone(),
                            span: to_byte_span(attr.key.1),
                        });
                        if attr.key.0 == "description"
                            && let Expr::Str(s) = &attr.value.0
                        {
                            description = Some(s.clone());
                        } else if attr.key.0 == "entities"
                            && let Expr::List(items) = &attr.value.0
                        {
                            for (expr, _) in items {
                                if let Some(target) = expr_as_name(expr) {
                                    entities.push(target.to_owned());
                                }
                            }
                        }
                    }
                    idx.register_decl(SymbolDecl {
                        kind: SymbolKind::Event,
                        name: name.0.clone(),
                        name_span,
                        full_span,
                        description: description.clone(),
                        graph_props: Vec::new(),
                        entities,
                    });
                    idx.top_decls.push(TopDeclEntry {
                        kind: "event",
                        name: name.0.clone(),
                        detail: description,
                        name_span,
                        full_span,
                    });
                }
                TopItem::Events { name, .. } => {
                    let name_span = to_byte_span(name.1);
                    idx.register_decl(SymbolDecl {
                        kind: SymbolKind::Events,
                        name: name.0.clone(),
                        name_span,
                        full_span,
                        description: None,
                        graph_props: Vec::new(),
                        entities: Vec::new(),
                    });
                    idx.top_decls.push(TopDeclEntry {
                        kind: "events",
                        name: name.0.clone(),
                        detail: None,
                        name_span,
                        full_span,
                    });
                }
            }
        }

        // 第二遍：收集全部引用出现位置（含统一节点引用与时间表达式引用）
        for (item, _) in &ast.items {
            match item {
                TopItem::Story(_) => {}
                TopItem::TimeAnchor { expr, .. } => {
                    idx.collect_time_expr_refs(expr, false);
                }
                TopItem::Entity { attrs, .. } => {
                    for (attr, _) in attrs {
                        match attr.key.0.as_str() {
                            "calculable_props" => {
                                if let Expr::List(items) = &attr.value.0 {
                                    for (expr, span) in items {
                                        if let Some(prop) = expr_as_name(expr) {
                                            idx.push_ref(
                                                SymbolKind::CalculableProp,
                                                prop,
                                                to_byte_span(*span),
                                            );
                                        }
                                    }
                                }
                            }
                            "base_time" => {
                                idx.collect_time_expr_refs(&attr.value, false);
                            }
                            _ => {}
                        }
                    }
                }
                TopItem::Link { name, edges, .. } => {
                    for (edge, edge_span) in edges {
                        let label = edge.label.as_ref().map(|(s, _)| s.clone());
                        let directed = edge.is_directed();
                        let (summary, sample_from, sample_to) = match &edge.kind {
                            LinkEdgeKind::Directed { left, right } => {
                                for (n, sp) in left.names() {
                                    let kind = idx.node_kind(n);
                                    idx.push_ref(kind, n, to_byte_span(*sp));
                                }
                                for (n, sp) in right.names() {
                                    let kind = idx.node_kind(n);
                                    idx.push_ref(kind, n, to_byte_span(*sp));
                                }
                                let l = format_endpoint_summary(left);
                                let r = format_endpoint_summary(right);
                                let op = match &label {
                                    Some(lbl) => format!("-- \"{lbl}\" ->"),
                                    None => "->".to_owned(),
                                };
                                (
                                    format!("{l} {op} {r}"),
                                    left.names().first().map(|(s, _)| s.clone()),
                                    right.names().first().map(|(s, _)| s.clone()),
                                )
                            }
                            LinkEdgeKind::Symmetric { left, right } => {
                                for (n, sp) in left.names() {
                                    let kind = idx.node_kind(n);
                                    idx.push_ref(kind, n, to_byte_span(*sp));
                                }
                                for (n, sp) in right.names() {
                                    let kind = idx.node_kind(n);
                                    idx.push_ref(kind, n, to_byte_span(*sp));
                                }
                                let l = format_endpoint_summary(left);
                                let r = format_endpoint_summary(right);
                                let op = match &label {
                                    Some(lbl) => format!("<- \"{lbl}\" ->"),
                                    None => "<->".to_owned(),
                                };
                                (
                                    format!("{l} {op} {r}"),
                                    left.names().first().map(|(s, _)| s.clone()),
                                    right.names().first().map(|(s, _)| s.clone()),
                                )
                            }
                            LinkEdgeKind::Clique { members, .. } => {
                                for (n, sp) in members {
                                    let kind = idx.node_kind(n);
                                    idx.push_ref(kind, n, to_byte_span(*sp));
                                }
                                let list_str = members
                                    .iter()
                                    .map(|(s, _)| s.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", ");
                                let op = match &label {
                                    Some(lbl) => format!("<- \"{lbl}\" ->"),
                                    None => "<->".to_owned(),
                                };
                                (
                                    format!("{op} [{list_str}]"),
                                    members.first().map(|(s, _)| s.clone()),
                                    members.get(1).map(|(s, _)| s.clone()),
                                )
                            }
                        };

                        if let Some((attrs, _)) = &edge.attrs {
                            for (attr, _) in attrs {
                                idx.attr_keys.push(AttrKeySite {
                                    block: BlockKind::LinkEdge,
                                    key: attr.key.0.clone(),
                                    span: to_byte_span(attr.key.1),
                                });
                                if matches!(attr.key.0.as_str(), "start" | "end") {
                                    idx.collect_time_expr_refs(&attr.value, true);
                                }
                            }
                        }

                        idx.link_edges.push(LinkEdgeSite {
                            graph: name.0.clone(),
                            summary,
                            sample_from,
                            sample_to,
                            directed,
                            label,
                            span: to_byte_span(*edge_span),
                        });
                    }
                }
                TopItem::Event { attrs, .. } => {
                    for (attr, _) in attrs {
                        match attr.key.0.as_str() {
                            "start" | "end" => {
                                idx.collect_time_expr_refs(&attr.value, true);
                            }
                            "entities" => {
                                if let Expr::List(items) = &attr.value.0 {
                                    for (expr, span) in items {
                                        if let Some(target) = expr_as_name(expr) {
                                            idx.push_ref(
                                                SymbolKind::Entity,
                                                target,
                                                to_byte_span(*span),
                                            );
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                TopItem::Events { events, .. } => {
                    for (ev_name, ev_span) in events {
                        idx.push_ref(SymbolKind::Event, ev_name, to_byte_span(*ev_span));
                    }
                }
            }
        }

        idx
    }

    fn node_kind(&self, name: &str) -> SymbolKind {
        if self
            .declarations
            .contains_key(&(SymbolKind::Event, name.to_owned()))
        {
            SymbolKind::Event
        } else {
            SymbolKind::Entity
        }
    }

    fn build_fallback_from_tokens(src: &str) -> Self {
        let mut idx = Self::default();
        let scanned = tokenize(src, 0);
        let tokens = &scanned.tokens;
        let mut brace_depth = 0usize;
        let mut bracket_depth = 0usize;
        let mut i = 0usize;
        while i < tokens.len() {
            let tok = &tokens[i];
            if brace_depth == 0 && bracket_depth == 0 && tok.kind == TokKind::Ident {
                let kind_pair = match tok.value.as_str() {
                    "entity" => Some((SymbolKind::Entity, "entity", TokKind::LBrace)),
                    "event" => Some((SymbolKind::Event, "event", TokKind::LBrace)),
                    "link" => Some((SymbolKind::LinkGraph, "link", TokKind::LBrace)),
                    "events" => Some((SymbolKind::Events, "events", TokKind::Eq)),
                    "time" => Some((SymbolKind::TimeAnchor, "time", TokKind::Eq)),
                    _ => None,
                };
                if let Some((sym_kind, kind_str, follow)) = kind_pair
                    && let Some(name_tok) = tokens.get(i + 1)
                    && name_tok.is_name()
                    && tokens.get(i + 2).is_some_and(|t| t.kind == follow)
                {
                    let full_span = ByteSpan::new(tok.span.start, name_tok.span.end);
                    idx.register_decl(SymbolDecl {
                        kind: sym_kind,
                        name: name_tok.value.clone(),
                        name_span: name_tok.span,
                        full_span,
                        description: None,
                        graph_props: Vec::new(),
                        entities: Vec::new(),
                    });
                    idx.top_decls.push(TopDeclEntry {
                        kind: kind_str,
                        name: name_tok.value.clone(),
                        detail: None,
                        name_span: name_tok.span,
                        full_span,
                    });
                }
            }
            match tok.kind {
                TokKind::LBrace => brace_depth += 1,
                TokKind::RBrace => brace_depth = brace_depth.saturating_sub(1),
                TokKind::LBracket => bracket_depth += 1,
                TokKind::RBracket => bracket_depth = bracket_depth.saturating_sub(1),
                _ => {}
            }
            i += 1;
        }
        idx
    }

    fn register_decl(&mut self, decl: SymbolDecl) {
        let key = (decl.kind, decl.name.clone());
        let is_first = !self.declarations.contains_key(&key);
        self.occurrences.push(Occurrence {
            kind: decl.kind,
            name: decl.name.clone(),
            span: decl.name_span,
            is_definition: true,
        });
        if is_first {
            self.declarations.insert(key, decl);
        }
    }

    fn push_ref(&mut self, kind: SymbolKind, name: &str, span: ByteSpan) {
        self.occurrences.push(Occurrence {
            kind,
            name: name.to_owned(),
            span,
            is_definition: false,
        });
    }

    fn collect_time_expr_refs(&mut self, expr: &Spanned<Expr>, inside_timed_block: bool) {
        match &expr.0 {
            Expr::Ident(name) => {
                if inside_timed_block && matches!(name.as_str(), "start" | "end") {
                    return;
                }
                self.push_ref(SymbolKind::TimeAnchor, name, to_byte_span(expr.1));
            }
            Expr::Member { base, field } => {
                let kind = match field.0.as_str() {
                    "base_time" => SymbolKind::Entity,
                    "start" | "end" => SymbolKind::Event,
                    _ => return,
                };
                self.push_ref(kind, &base.0, to_byte_span(base.1));
            }
            Expr::Binary { lhs, rhs, .. } => {
                self.collect_time_expr_refs(lhs, inside_timed_block);
                self.collect_time_expr_refs(rhs, inside_timed_block);
            }
            _ => {}
        }
    }

    pub fn occurrence_at(&self, byte_offset: usize) -> Option<&Occurrence> {
        self.occurrences
            .iter()
            .find(|occ| occ.span.start <= byte_offset && byte_offset <= occ.span.end)
    }

    pub fn declaration(&self, kind: SymbolKind, name: &str) -> Option<&SymbolDecl> {
        self.declarations.get(&(kind, name.to_owned()))
    }

    pub fn occurrences_of(&self, kind: SymbolKind, name: &str) -> Vec<&Occurrence> {
        self.occurrences
            .iter()
            .filter(|occ| occ.kind == kind && occ.name == name)
            .collect()
    }

    pub fn attr_key_at(&self, byte_offset: usize) -> Option<&AttrKeySite> {
        self.attr_keys
            .iter()
            .find(|site| site.span.start <= byte_offset && byte_offset <= site.span.end)
    }

    pub fn link_edge_at(&self, byte_offset: usize) -> Option<&LinkEdgeSite> {
        self.link_edges
            .iter()
            .find(|edge| edge.span.start <= byte_offset && byte_offset <= edge.span.end)
    }
}

fn expr_as_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Ident(s) | Expr::Str(s) => Some(s.as_str()),
        _ => None,
    }
}

fn format_endpoint_summary(endpoint: &LinkEndpoint) -> String {
    match endpoint {
        LinkEndpoint::Single((name, _)) => name.clone(),
        LinkEndpoint::List(names, _) => {
            let joined = names
                .iter()
                .map(|(s, _)| s.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{joined}]")
        }
    }
}
