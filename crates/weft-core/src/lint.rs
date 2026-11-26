//! 静态 Lint 规则与拼写建议工具。
//!
//! 所有规则产生 `Severity::Warning` 诊断，可通过 CLI `--ignore <rule>` 按名屏蔽。

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::entity::{RawEntity, RawEntityLink};
use crate::errors::{ByteSpan, Diagnostic};
use crate::event::RawEvent;

/// 所有可选 Lint 规则名清单。
pub const LINT_RULES: &[&str] = &[
    "duplicate-attr",
    "unused-time-anchor",
    "unused-entity",
    "unreferenced-event",
    "self-link",
    "duplicate-link",
    "empty-block",
];

pub(crate) struct LintContext<'a> {
    pub anchors: &'a BTreeMap<String, ByteSpan>,
    pub used_anchors: &'a HashSet<String>,
    pub entities: &'a BTreeMap<String, RawEntity>,
    pub links: &'a BTreeMap<String, Vec<RawEntityLink>>,
    pub link_group_spans: &'a HashMap<String, ByteSpan>,
    pub events: &'a BTreeMap<String, RawEvent>,
    pub event_lists: &'a BTreeMap<String, (ByteSpan, Vec<(String, ByteSpan)>)>,
}

pub(crate) fn run_lints(ctx: &LintContext<'_>, diagnostics: &mut Vec<Diagnostic>) {
    lint_unused_anchors(ctx, diagnostics);
    lint_links(ctx, diagnostics);
    lint_unused_entities(ctx, diagnostics);
    lint_event_lists_and_events(ctx, diagnostics);
}

fn lint_unused_anchors(ctx: &LintContext<'_>, diagnostics: &mut Vec<Diagnostic>) {
    for (name, span) in ctx.anchors {
        if !ctx.used_anchors.contains(name) {
            diagnostics.push(Diagnostic::warning(
                "unused-time-anchor",
                format!("time anchor `{name}` is declared but never used"),
                Some(*span),
            ));
        }
    }
}

fn lint_links(ctx: &LintContext<'_>, diagnostics: &mut Vec<Diagnostic>) {
    for (group, edges) in ctx.links {
        if edges.is_empty() {
            diagnostics.push(Diagnostic::warning(
                "empty-block",
                format!("link block `{group}` is empty"),
                ctx.link_group_spans.get(group).copied(),
            ));
            continue;
        }
        let mut seen: HashMap<(&str, &str, &str), ByteSpan> = HashMap::new();
        for edge in edges {
            if edge.left.0 == edge.right.0 {
                diagnostics.push(Diagnostic::warning(
                    "self-link",
                    format!(
                        "link `{group}` connects entity `{}` to itself",
                        edge.left.0
                    ),
                    Some(edge.span),
                ));
            }
            let key = (
                edge.left.0.as_str(),
                edge.relations.as_str(),
                edge.right.0.as_str(),
            );
            if let Some(&first_span) = seen.get(&key) {
                diagnostics.push(
                    Diagnostic::warning(
                        "duplicate-link",
                        format!(
                            "duplicate link edge `{} -- \"{}\" -- {}` in `{group}`",
                            edge.left.0, edge.relations, edge.right.0
                        ),
                        Some(edge.span),
                    )
                    .with_related(first_span, "first declared here"),
                );
            } else {
                seen.insert(key, edge.span);
            }
        }
    }
}

fn lint_unused_entities(ctx: &LintContext<'_>, diagnostics: &mut Vec<Diagnostic>) {
    if ctx.events.is_empty() && ctx.links.is_empty() {
        return;
    }
    let mut referenced: HashSet<&str> = HashSet::new();
    for raw in ctx.entities.values() {
        for (parent, _) in &raw.belong_to {
            referenced.insert(parent.as_str());
        }
    }
    for edges in ctx.links.values() {
        for edge in edges {
            referenced.insert(edge.left.0.as_str());
            referenced.insert(edge.right.0.as_str());
        }
    }
    for ev in ctx.events.values() {
        for (ent, _) in &ev.entities {
            referenced.insert(ent.as_str());
        }
    }
    for (id, raw) in ctx.entities {
        if !referenced.contains(id.as_str()) {
            diagnostics.push(Diagnostic::warning(
                "unused-entity",
                format!("entity `{id}` is never referenced by any event, link, or belong_to"),
                Some(raw.name_span),
            ));
        }
    }
}

fn lint_event_lists_and_events(ctx: &LintContext<'_>, diagnostics: &mut Vec<Diagnostic>) {
    let mut referenced_events: HashSet<&str> = HashSet::new();
    for (name, (name_span, refs)) in ctx.event_lists {
        if refs.is_empty() {
            diagnostics.push(Diagnostic::warning(
                "empty-block",
                format!("events `{name}` is empty"),
                Some(*name_span),
            ));
        }
        for (ev, _) in refs {
            referenced_events.insert(ev.as_str());
        }
    }

    if ctx.event_lists.is_empty() {
        return;
    }
    for (id, raw) in ctx.events {
        if !referenced_events.contains(id.as_str()) {
            diagnostics.push(Diagnostic::warning(
                "unreferenced-event",
                format!("event `{id}` is not included in any events"),
                Some(raw.name_span),
            ));
        }
    }
}

/// 在候选列表中寻找编辑距离最近的拼写建议（用于 "did you mean ...?"）。
pub(crate) fn suggest_similar<'a>(needle: &str, candidates: &[&'a str]) -> Option<&'a str> {
    let needle_chars: Vec<char> = needle.chars().collect();
    if needle_chars.is_empty() {
        return None;
    }
    let max_dist = (needle_chars.len() / 2).clamp(1, 3);
    let mut best: Option<(&'a str, usize)> = None;
    for &cand in candidates {
        let cand_chars: Vec<char> = cand.chars().collect();
        if needle_chars.len().abs_diff(cand_chars.len()) > max_dist {
            continue;
        }
        let dist = levenshtein(&needle_chars, &cand_chars);
        if dist > 0 && dist <= max_dist && best.is_none_or(|(_, d)| dist < d) {
            best = Some((cand, dist));
        }
    }
    best.map(|(cand, _)| cand)
}

fn levenshtein(a: &[char], b: &[char]) -> usize {
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0; b.len() + 1];
    for (i, &ac) in a.iter().enumerate() {
        curr[0] = i + 1;
        for (j, &bc) in b.iter().enumerate() {
            let cost = usize::from(ac != bc);
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}
