//! 静态 Lint 规则与拼写建议工具。
//!
//! 所有规则产生 `Severity::Warning` 诊断，可通过 CLI `--ignore <rule>` 按名屏蔽。

use std::collections::{BTreeMap, HashSet};

use crate::entity::RawEntity;
use crate::errors::{ByteSpan, Diagnostic};
use crate::event::RawEvent;
use crate::relation::{RawLink, RawLinkGraph};

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
    pub lang: &'a str,
    pub anchors: &'a BTreeMap<String, ByteSpan>,
    pub used_anchors: &'a HashSet<String>,
    pub graphs: &'a BTreeMap<String, RawLinkGraph>,
    pub entities: &'a BTreeMap<String, RawEntity>,
    pub links: &'a [RawLink],
    pub events: &'a BTreeMap<String, RawEvent>,
    pub event_lists: &'a BTreeMap<String, (ByteSpan, Vec<(String, ByteSpan)>)>,
}

pub(crate) fn run_lints(ctx: &LintContext<'_>, diagnostics: &mut Vec<Diagnostic>) {
    lint_unused_anchors(ctx, diagnostics);
    lint_links_static(ctx, diagnostics);
    lint_unused_entities(ctx, diagnostics);
    lint_event_lists_and_events(ctx, diagnostics);
}

fn lint_unused_anchors(ctx: &LintContext<'_>, diagnostics: &mut Vec<Diagnostic>) {
    let zh = ctx.lang == "zh";
    for (name, span) in ctx.anchors {
        if !ctx.used_anchors.contains(name) {
            let msg = if zh {
                format!("时间锚点 `{name}` 已声明但从未被使用")
            } else {
                format!("time anchor `{name}` is declared but never used")
            };
            diagnostics.push(Diagnostic::warning("unused-time-anchor", msg, Some(*span)));
        }
    }
}

fn lint_links_static(ctx: &LintContext<'_>, diagnostics: &mut Vec<Diagnostic>) {
    let zh = ctx.lang == "zh";
    for (graph, raw) in ctx.graphs {
        if raw.edge_stmt_count == 0 {
            let msg = if zh {
                format!("关系图 `{graph}` 不包含任何边")
            } else {
                format!("link graph `{graph}` has no edges")
            };
            diagnostics.push(Diagnostic::warning(
                "empty-block",
                msg,
                Some(raw.name_span),
            ));
        }
    }
    for edge in ctx.links {
        if edge.from.0 == edge.to.0 {
            // 若所在图启用了 acyclic 或 asymmetric，图校验阶段已报告 Error，避免叠加重复 Warning。
            if ctx
                .graphs
                .get(&edge.graph)
                .is_some_and(|g| g.spec.acyclic || g.spec.asymmetric)
            {
                continue;
            }
            let msg = if zh {
                format!("关系图 `{}` 将节点 `{}` 连向了自身", edge.graph, edge.from.0)
            } else {
                format!(
                    "link `{}` connects node `{}` to itself",
                    edge.graph, edge.from.0
                )
            };
            diagnostics.push(Diagnostic::warning("self-link", msg, Some(edge.span)));
        }
    }
}

fn lint_unused_entities(ctx: &LintContext<'_>, diagnostics: &mut Vec<Diagnostic>) {
    if ctx.events.is_empty() && ctx.links.is_empty() {
        return;
    }
    let zh = ctx.lang == "zh";
    let mut referenced: HashSet<&str> = HashSet::new();
    for edge in ctx.links {
        referenced.insert(edge.from.0.as_str());
        referenced.insert(edge.to.0.as_str());
    }
    for ev in ctx.events.values() {
        for (ent, _) in &ev.entities {
            referenced.insert(ent.as_str());
        }
    }
    for (id, raw) in ctx.entities {
        if !referenced.contains(id.as_str()) {
            let msg = if zh {
                format!("实体 `{id}` 未被任何事件或 link 引用")
            } else {
                format!("entity `{id}` is never referenced by any event or link")
            };
            diagnostics.push(Diagnostic::warning("unused-entity", msg, Some(raw.name_span)));
        }
    }
}

fn lint_event_lists_and_events(ctx: &LintContext<'_>, diagnostics: &mut Vec<Diagnostic>) {
    let zh = ctx.lang == "zh";
    let mut referenced_events: HashSet<&str> = HashSet::new();
    for (name, (name_span, refs)) in ctx.event_lists {
        if refs.is_empty() {
            let msg = if zh {
                format!("事件列表 `{name}` 为空")
            } else {
                format!("events `{name}` is empty")
            };
            diagnostics.push(Diagnostic::warning("empty-block", msg, Some(*name_span)));
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
            let msg = if zh {
                format!("事件 `{id}` 未被任何事件列表收录")
            } else {
                format!("event `{id}` is not included in any events")
            };
            diagnostics.push(Diagnostic::warning(
                "unreferenced-event",
                msg,
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
