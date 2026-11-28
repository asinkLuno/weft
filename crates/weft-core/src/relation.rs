//! 关系图代数定义、双色同质分层校验、时变边模型与基于 `petgraph` 的原子时间切片图校验。

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use petgraph::{
    Direction,
    algo::tarjan_scc,
    graphmap::DiGraphMap,
    unionfind::UnionFind,
};
use serde::Serialize;

use crate::calendar::{Calendar, Moment};
use crate::entity::{Entity, RawEntity};
use crate::errors::{ByteSpan, Diagnostic, ValidationError};
use crate::event::{Event, RawEvent};
use crate::lint::suggest_similar;
use crate::phase::Phase;

/// `link` 图的同质节点域：一张关系图只能连接纯 `entity`（状态演化图）或纯 `event`（剧情拓扑图）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkDomain {
    Entity,
    Event,
}

impl LinkDomain {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Entity => "entity",
            Self::Event => "event",
        }
    }
}

// ---- 关系图结构模型（挂在每个 `link` 块上） ----

#[derive(Clone, Debug, Default, Serialize)]
pub struct LinkGraph {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<LinkDomain>,
    pub acyclic: bool,
    pub asymmetric: bool,
    pub intransitive: bool,
    pub functional: bool,
    pub inverse_functional: bool,
    pub symmetric: bool,
    pub transitive: bool,
    pub equivalence: bool,
    pub chronological: bool,
    pub description: Option<String>,
}

impl LinkGraph {
    /// 是否强制全部使用有向边（`->` / `-- "..." ->`）。
    pub fn requires_directed(&self) -> bool {
        self.acyclic || self.asymmetric || self.chronological
    }

    /// 是否强制全部使用无向对称边或团（`<->` / `<- "..." ->`）。
    pub fn requires_undirected(&self) -> bool {
        self.symmetric || self.equivalence
    }

    /// 是否需要进行原子时间区间图约束校验。
    pub fn needs_graph_check(&self) -> bool {
        self.acyclic
            || self.asymmetric
            || self.intransitive
            || self.functional
            || self.inverse_functional
    }

    /// 返回启用的代数/拓扑属性名清单（供 LSP Hover 与文档展示）。
    pub fn enabled_properties(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.acyclic {
            out.push("acyclic");
        }
        if self.asymmetric {
            out.push("asymmetric");
        }
        if self.intransitive {
            out.push("intransitive");
        }
        if self.functional {
            out.push("functional");
        }
        if self.inverse_functional {
            out.push("inverse_functional");
        }
        if self.symmetric {
            out.push("symmetric");
        }
        if self.transitive {
            out.push("transitive");
        }
        if self.equivalence {
            out.push("equivalence");
        }
        if self.chronological {
            out.push("chronological");
        }
        out
    }

    /// 检查互斥属性组合，返回所有冲突对 `(attr_a, attr_b)`。
    pub fn mutual_exclusion_conflicts(&self) -> Vec<(&'static str, &'static str)> {
        let mut conflicts = Vec::new();
        let pairs: [(&'static str, bool, &'static str, bool); 10] = [
            ("acyclic", self.acyclic, "symmetric", self.symmetric),
            ("acyclic", self.acyclic, "equivalence", self.equivalence),
            ("asymmetric", self.asymmetric, "symmetric", self.symmetric),
            ("asymmetric", self.asymmetric, "equivalence", self.equivalence),
            (
                "chronological",
                self.chronological,
                "symmetric",
                self.symmetric,
            ),
            (
                "chronological",
                self.chronological,
                "equivalence",
                self.equivalence,
            ),
            ("transitive", self.transitive, "intransitive", self.intransitive),
            ("equivalence", self.equivalence, "intransitive", self.intransitive),
            ("equivalence", self.equivalence, "functional", self.functional),
            (
                "equivalence",
                self.equivalence,
                "inverse_functional",
                self.inverse_functional,
            ),
        ];
        for (a, en_a, b, en_b) in pairs {
            if en_a && en_b {
                conflicts.push((a, b));
            }
        }
        conflicts
    }
}

#[derive(Debug)]
pub(crate) struct RawLinkGraph {
    pub name_span: ByteSpan,
    pub block_span: ByteSpan,
    pub spec: LinkGraph,
    pub attr_spans: HashMap<String, ByteSpan>,
    pub edge_stmt_count: usize,
}

// ---- 时变边模型 ----

#[derive(Debug)]
pub(crate) struct RawLink {
    pub stmt_id: usize,
    pub graph: String,
    pub from: (String, ByteSpan),
    pub to: (String, ByteSpan),
    /// `true` 表示有向边；`false` 表示无向对称边（含团展开出的边）。
    pub directed: bool,
    pub label: Option<(String, ByteSpan)>,
    pub start_attr_span: Option<ByteSpan>,
    pub end_attr_span: Option<ByteSpan>,
    pub start_time: Option<(Phase, ByteSpan)>,
    pub end_time: Option<(Phase, ByteSpan)>,
    pub description: Option<String>,
    pub span: ByteSpan,
}

#[derive(Clone, Debug, Serialize)]
pub struct LinkTime {
    pub raw: Vec<i64>,
    pub formatted: String,
    pub tick: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Link {
    pub graph: String,
    pub from: String,
    pub to: String,
    pub directed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub start: Option<LinkTime>,
    pub end: Option<LinkTime>,
    pub description: Option<String>,
}

pub(crate) fn format_edge_repr(
    from: &str,
    to: &str,
    directed: bool,
    label: Option<&str>,
) -> String {
    match (directed, label) {
        (true, None) => format!("{from} -> {to}"),
        (true, Some(l)) => format!("{from} -- \"{l}\" -> {to}"),
        (false, None) => format!("{from} <-> {to}"),
        (false, Some(l)) => format!("{from} <- \"{l}\" -> {to}"),
    }
}

// ---- 静态校验（不依赖历法脚本） ----

/// 校验所有 `link` 图的属性互斥约束，返回配置合法的图名集合。
pub(crate) fn validate_graphs_static(
    graphs: &BTreeMap<String, RawLinkGraph>,
    diagnostics: &mut Vec<Diagnostic>,
) -> HashSet<String> {
    let mut valid = HashSet::with_capacity(graphs.len());
    for (name, raw) in graphs {
        let conflicts = raw.spec.mutual_exclusion_conflicts();
        if conflicts.is_empty() {
            valid.insert(name.clone());
            continue;
        }
        for (attr_a, attr_b) in conflicts {
            let primary_span = raw
                .attr_spans
                .get(attr_b)
                .copied()
                .unwrap_or(raw.block_span);
            let mut diag = Diagnostic::from_validation(
                ValidationError::InvalidGraphConfig {
                    graph: name.clone(),
                    attr_a: attr_a.to_owned(),
                    attr_b: attr_b.to_owned(),
                },
                Some(primary_span),
            );
            if let Some(&span_a) = raw.attr_spans.get(attr_a) {
                diag = diag.with_related(span_a, format!("`{attr_a} = true` enabled here"));
            }
            diagnostics.push(diag);
        }
    }
    valid
}

/// 校验所有 `link` 边的端点存在性、同质分层（纯 entity 图 vs 纯 event 图）、时变属性合法性与箭头方向。
/// 同时将推断出的 `LinkDomain` 写回 `graphs`，返回每条展开边是否通过静态合法性检查。
pub(crate) fn validate_links_static(
    links: &[RawLink],
    graphs: &mut BTreeMap<String, RawLinkGraph>,
    valid_graphs: &mut HashSet<String>,
    entities: &BTreeMap<String, RawEntity>,
    events: &BTreeMap<String, RawEvent>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<bool> {
    let mut node_names: Vec<&str> = entities
        .keys()
        .chain(events.keys())
        .map(String::as_str)
        .collect();
    node_names.sort_unstable();
    node_names.dedup();

    let mut valid_flags = Vec::with_capacity(links.len());
    let mut graph_domains: HashMap<String, (LinkDomain, ByteSpan)> = HashMap::new();
    let mut reported_unknown: HashSet<(usize, ByteSpan)> = HashSet::new();
    let mut reported_mixed: HashSet<(usize, ByteSpan)> = HashSet::new();
    let mut reported_dir: HashSet<usize> = HashSet::new();
    let mut reported_time_bounds: HashSet<usize> = HashSet::new();
    let mut reported_ev_equiv: HashSet<String> = HashSet::new();

    for link in links {
        let mut ok = true;
        for (node_id, node_span) in [&link.from, &link.to] {
            let kind = if entities.contains_key(node_id) {
                Some(LinkDomain::Entity)
            } else if events.contains_key(node_id) {
                Some(LinkDomain::Event)
            } else {
                None
            };

            let Some(kind) = kind else {
                ok = false;
                if reported_unknown.insert((link.stmt_id, *node_span)) {
                    let mut diag = Diagnostic::from_validation(
                        ValidationError::UnknownLinkEntity {
                            graph: link.graph.clone(),
                            entity: node_id.clone(),
                        },
                        Some(*node_span),
                    );
                    if let Some(suggestion) = suggest_similar(node_id, &node_names) {
                        diag = diag.with_suggestion(suggestion);
                    }
                    diagnostics.push(diag);
                }
                continue;
            };

            match graph_domains.get(&link.graph).copied() {
                None => {
                    graph_domains.insert(link.graph.clone(), (kind, *node_span));
                }
                Some((expected, first_span)) if expected != kind => {
                    ok = false;
                    if reported_mixed.insert((link.stmt_id, *node_span)) {
                        let diag = Diagnostic::from_validation(
                            ValidationError::MixedLinkNodeKinds {
                                graph: link.graph.clone(),
                                expected_kind: expected.as_str().to_owned(),
                                found_kind: kind.as_str().to_owned(),
                                node: node_id.clone(),
                            },
                            Some(*node_span),
                        )
                        .with_related(
                            first_span,
                            format!("graph `{}` first used {} node here", link.graph, expected.as_str()),
                        );
                        diagnostics.push(diag);
                    }
                }
                Some(_) => {}
            }
        }

        let domain = graph_domains.get(&link.graph).map(|(d, _)| *d);

        if let Some(graph_def) = graphs.get(&link.graph) {
            if domain == Some(LinkDomain::Event) && graph_def.spec.equivalence {
                ok = false;
                valid_graphs.remove(&link.graph);
                if reported_ev_equiv.insert(link.graph.clone()) {
                    let span = graph_def
                        .attr_spans
                        .get("equivalence")
                        .copied()
                        .unwrap_or(graph_def.name_span);
                    diagnostics.push(Diagnostic::from_validation(
                        ValidationError::EventGraphEquivalence {
                            graph: link.graph.clone(),
                        },
                        Some(span),
                    ));
                }
            }

            let time_bound_span = link.start_attr_span.or(link.end_attr_span);
            if let Some(tb_span) = time_bound_span {
                if domain == Some(LinkDomain::Event) {
                    ok = false;
                    if reported_time_bounds.insert(link.stmt_id) {
                        diagnostics.push(Diagnostic::from_validation(
                            ValidationError::EventLinkHasTimeBounds {
                                graph: link.graph.clone(),
                            },
                            Some(tb_span),
                        ));
                    }
                } else if graph_def.spec.equivalence {
                    ok = false;
                    if reported_time_bounds.insert(link.stmt_id) {
                        diagnostics.push(Diagnostic::from_validation(
                            ValidationError::EquivalenceLinkHasTimeBounds {
                                graph: link.graph.clone(),
                            },
                            Some(tb_span),
                        ));
                    }
                }
            }

            if valid_graphs.contains(&link.graph) {
                if graph_def.spec.requires_undirected() && link.directed {
                    ok = false;
                    if reported_dir.insert(link.stmt_id) {
                        diagnostics.push(Diagnostic::from_validation(
                            ValidationError::ArrowDirectionMismatch {
                                graph: link.graph.clone(),
                                expected: "`<->` or `<- \"...\" ->`".to_owned(),
                            },
                            Some(link.span),
                        ));
                    }
                } else if graph_def.spec.requires_directed() && !link.directed {
                    ok = false;
                    if reported_dir.insert(link.stmt_id) {
                        diagnostics.push(Diagnostic::from_validation(
                            ValidationError::ArrowDirectionMismatch {
                                graph: link.graph.clone(),
                                expected: "`->` or `-- \"...\" ->`".to_owned(),
                            },
                            Some(link.span),
                        ));
                    }
                }
            }
        }

        valid_flags.push(ok);
    }

    for (graph_name, (domain, _)) in graph_domains {
        if let Some(raw_graph) = graphs.get_mut(&graph_name) {
            raw_graph.spec.domain = Some(domain);
        }
    }

    valid_flags
}

// ---- 运行时构建 + 原子时间切片图校验 + 时序单调性校验 ----

#[derive(Clone, Copy, Debug)]
struct ResolvedEdge<'a> {
    idx: usize,
    graph: &'a str,
    from: &'a str,
    to: &'a str,
    directed: bool,
    label: Option<&'a str>,
    start_tick: Option<i64>,
    end_tick: Option<i64>,
    span: ByteSpan,
    time_valid: bool,
    valid_for_graph: bool,
}

#[derive(Clone, Copy, Debug)]
enum AtomicSlice {
    /// 左闭右开区间 `[start, end)`（`None` 分别代表 $-\infty$ / $+\infty$）
    HalfOpen { start: Option<i64>, end: Option<i64> },
    /// 瞬时时间点 `[t, t]`（处理 `start == end` 的瞬时边）
    Point(i64),
}

impl AtomicSlice {
    fn contains_edge(self, edge_start: Option<i64>, edge_end: Option<i64>) -> bool {
        match self {
            AtomicSlice::HalfOpen { start, end } => {
                if let (Some(es), Some(ee)) = (edge_start, edge_end)
                    && es == ee
                {
                    return false;
                }
                let starts_before = match (edge_start, start) {
                    (None, _) => true,
                    (Some(_), None) => false,
                    (Some(es), Some(s)) => es <= s,
                };
                let ends_after = match (edge_end, end) {
                    (None, _) => true,
                    (Some(_), None) => false,
                    (Some(ee), Some(e)) => ee >= e,
                };
                starts_before && ends_after
            }
            AtomicSlice::Point(t) => {
                let starts_ok = edge_start.is_none_or(|es| es <= t);
                let ends_ok = match (edge_start, edge_end) {
                    (Some(es), Some(ee)) if es == ee => es == t,
                    (_, Some(ee)) => t < ee,
                    (_, None) => true,
                };
                starts_ok && ends_ok
            }
        }
    }
}

pub(crate) struct LinkRuntimeContext<'a> {
    pub graphs: &'a BTreeMap<String, RawLinkGraph>,
    pub valid_graphs: &'a HashSet<String>,
    pub entities: &'a HashMap<String, Entity>,
    pub entity_base_times: &'a HashMap<String, Moment>,
    pub events: &'a BTreeMap<String, Event>,
    pub calendar: &'a Calendar,
    pub lang: &'a str,
}

/// 求解所有 `link` 边的时间区间，构造运行时 `Vec<Link>`，并执行原子时间切片图校验、时序单调性校验与时变查重。
pub(crate) fn build_and_validate_links_runtime(
    raw_links: &[RawLink],
    static_valid: &[bool],
    ctx: &LinkRuntimeContext<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<Link> {
    let mut out = Vec::with_capacity(raw_links.len());
    let mut resolved_edges = Vec::with_capacity(raw_links.len());
    let mut stmt_time_cache: HashMap<usize, (Option<LinkTime>, Option<LinkTime>, bool)> =
        HashMap::new();

    for (idx, raw) in raw_links.iter().enumerate() {
        let suppress_time = !static_valid[idx]
            && (raw.start_attr_span.is_some() || raw.end_attr_span.is_some())
            && ctx.graphs.get(&raw.graph).is_some_and(|g| {
                g.spec.domain == Some(LinkDomain::Event) || g.spec.equivalence
            });

        let (start, end, time_ok) = if suppress_time {
            (None, None, false)
        } else if let Some(cached) = stmt_time_cache.get(&raw.stmt_id) {
            cached.clone()
        } else {
            let mut time_ok = true;
            let start = match &raw.start_time {
                Some((phase, span)) => match ctx.calendar.resolve(phase) {
                    Ok(moment) => match ctx.calendar.humanize(&moment) {
                        Ok(formatted) => Some(LinkTime {
                            raw: moment.values,
                            formatted,
                            tick: moment.tick,
                        }),
                        Err(err) => {
                            diagnostics.push(err.into_diagnostic(Some(*span)));
                            time_ok = false;
                            None
                        }
                    },
                    Err(err) => {
                        diagnostics.push(err.into_diagnostic(Some(*span)));
                        time_ok = false;
                        None
                    }
                },
                None => None,
            };

            let end = match &raw.end_time {
                Some((phase, span)) => match ctx.calendar.resolve(phase) {
                    Ok(moment) => {
                        if let Some(s) = &start
                            && moment.tick < s.tick
                        {
                            let edge_repr = format_edge_repr(
                                &raw.from.0,
                                &raw.to.0,
                                raw.directed,
                                raw.label.as_ref().map(|(l, _)| l.as_str()),
                            );
                            let mut diag = Diagnostic::from_validation(
                                ValidationError::LinkEndBeforeStart {
                                    graph: raw.graph.clone(),
                                    edge: edge_repr,
                                },
                                Some(*span),
                            );
                            if let Some((_, start_span)) = raw.start_time {
                                diag = diag.with_related(start_span, "link starts here");
                            }
                            diagnostics.push(diag);
                            time_ok = false;
                            None
                        } else {
                            match ctx.calendar.humanize(&moment) {
                                Ok(formatted) => Some(LinkTime {
                                    raw: moment.values,
                                    formatted,
                                    tick: moment.tick,
                                }),
                                Err(err) => {
                                    diagnostics.push(err.into_diagnostic(Some(*span)));
                                    time_ok = false;
                                    None
                                }
                            }
                        }
                    }
                    Err(err) => {
                        diagnostics.push(err.into_diagnostic(Some(*span)));
                        time_ok = false;
                        None
                    }
                },
                None => None,
            };

            let entry = (start, end, time_ok);
            stmt_time_cache.insert(raw.stmt_id, entry.clone());
            entry
        };

        let start_tick = start.as_ref().map(|t| t.tick);
        let end_tick = end.as_ref().map(|t| t.tick);
        resolved_edges.push(ResolvedEdge {
            idx,
            graph: &raw.graph,
            from: &raw.from.0,
            to: &raw.to.0,
            directed: raw.directed,
            label: raw.label.as_ref().map(|(l, _)| l.as_str()),
            start_tick,
            end_tick,
            span: raw.span,
            time_valid: time_ok,
            valid_for_graph: static_valid[idx] && time_ok,
        });

        out.push(Link {
            graph: raw.graph.clone(),
            from: raw.from.0.clone(),
            to: raw.to.0.clone(),
            directed: raw.directed,
            label: raw.label.as_ref().map(|(l, _)| l.clone()),
            start,
            end,
            description: raw.description.clone(),
        });
    }

    let mut tick_labels: HashMap<i64, String> = HashMap::new();
    for link in &out {
        if let Some(s) = &link.start {
            tick_labels.entry(s.tick).or_insert_with(|| s.formatted.clone());
        }
        if let Some(e) = &link.end {
            tick_labels.entry(e.tick).or_insert_with(|| e.formatted.clone());
        }
    }

    run_temporal_graph_checks(
        &resolved_edges,
        ctx.graphs,
        ctx.valid_graphs,
        &tick_labels,
        diagnostics,
    );
    run_chronological_checks(&resolved_edges, ctx, diagnostics);
    lint_duplicate_links_temporal(&resolved_edges, ctx.lang, diagnostics);

    out
}

/// `--syntax-only` 模式下对无时间边界边执行静态图校验与查重（不加载外部 `.rn` 历法）。
pub(crate) fn validate_timeless_links_fallback(
    raw_links: &[RawLink],
    static_valid: &[bool],
    graphs: &BTreeMap<String, RawLinkGraph>,
    valid_graphs: &HashSet<String>,
    lang: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let resolved_edges: Vec<ResolvedEdge<'_>> = raw_links
        .iter()
        .enumerate()
        .filter(|(_, raw)| raw.start_time.is_none() && raw.end_time.is_none())
        .map(|(idx, raw)| ResolvedEdge {
            idx,
            graph: &raw.graph,
            from: &raw.from.0,
            to: &raw.to.0,
            directed: raw.directed,
            label: raw.label.as_ref().map(|(l, _)| l.as_str()),
            start_tick: None,
            end_tick: None,
            span: raw.span,
            time_valid: true,
            valid_for_graph: static_valid[idx],
        })
        .collect();

    let empty_labels = HashMap::new();
    run_temporal_graph_checks(
        &resolved_edges,
        graphs,
        valid_graphs,
        &empty_labels,
        diagnostics,
    );
    lint_duplicate_links_temporal(&resolved_edges, lang, diagnostics);
}

fn run_chronological_checks(
    resolved_edges: &[ResolvedEdge<'_>],
    ctx: &LinkRuntimeContext<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut reported: HashSet<(&str, &str, &str, ByteSpan)> = HashSet::new();
    for edge in resolved_edges {
        if !edge.valid_for_graph || !edge.directed || !ctx.valid_graphs.contains(edge.graph) {
            continue;
        }
        let Some(raw_graph) = ctx.graphs.get(edge.graph) else {
            continue;
        };
        if !raw_graph.spec.chronological {
            continue;
        }

        let pair_times: Option<(i64, &str, i64, &str)> = match raw_graph.spec.domain {
            Some(LinkDomain::Event) => {
                if let (Some(ev_from), Some(ev_to)) =
                    (ctx.events.get(edge.from), ctx.events.get(edge.to))
                {
                    Some((
                        ev_from.start_tick,
                        ev_from.start_display.as_str(),
                        ev_to.start_tick,
                        ev_to.start_display.as_str(),
                    ))
                } else {
                    None
                }
            }
            Some(LinkDomain::Entity) => {
                if let (Some(m_from), Some(m_to), Some(ent_from), Some(ent_to)) = (
                    ctx.entity_base_times.get(edge.from),
                    ctx.entity_base_times.get(edge.to),
                    ctx.entities.get(edge.from),
                    ctx.entities.get(edge.to),
                ) && let (Some(disp_from), Some(disp_to)) = (
                    ent_from.base_time_humanizer.as_deref(),
                    ent_to.base_time_humanizer.as_deref(),
                ) {
                    Some((m_from.tick, disp_from, m_to.tick, disp_to))
                } else {
                    None
                }
            }
            None => None,
        };

        if let Some((from_tick, from_disp, to_tick, to_disp)) = pair_times
            && from_tick > to_tick
            && reported.insert((edge.graph, edge.from, edge.to, edge.span))
        {
            diagnostics.push(Diagnostic::from_validation(
                ValidationError::ChronologicalViolation {
                    graph: edge.graph.to_owned(),
                    from: edge.from.to_owned(),
                    from_time: from_disp.to_owned(),
                    to: edge.to.to_owned(),
                    to_time: to_disp.to_owned(),
                },
                Some(edge.span),
            ));
        }
    }
}

fn format_slice(slice: AtomicSlice, labels: &HashMap<i64, String>) -> String {
    let fmt = |tick: Option<i64>, inf: &str| match tick {
        Some(t) => labels
            .get(&t)
            .cloned()
            .unwrap_or_else(|| t.to_string()),
        None => inf.to_owned(),
    };
    match slice {
        AtomicSlice::HalfOpen { start, end } => {
            format!("[{}, {})", fmt(start, "-∞"), fmt(end, "+∞"))
        }
        AtomicSlice::Point(t) => {
            let s = fmt(Some(t), "");
            format!("[{s}, {s}]")
        }
    }
}

fn build_atomic_slices(edges: &[ResolvedEdge<'_>]) -> Vec<AtomicSlice> {
    let mut bounds: BTreeSet<i64> = BTreeSet::new();
    let mut point_ticks: BTreeSet<i64> = BTreeSet::new();
    for edge in edges {
        if let Some(s) = edge.start_tick {
            bounds.insert(s);
        }
        if let Some(e) = edge.end_tick {
            bounds.insert(e);
        }
        if let (Some(s), Some(e)) = (edge.start_tick, edge.end_tick)
            && s == e
        {
            point_ticks.insert(s);
        }
    }

    let sorted: Vec<i64> = bounds.into_iter().collect();
    let mut slices = Vec::new();
    if sorted.is_empty() {
        slices.push(AtomicSlice::HalfOpen {
            start: None,
            end: None,
        });
    } else {
        slices.push(AtomicSlice::HalfOpen {
            start: None,
            end: Some(sorted[0]),
        });
        for window in sorted.windows(2) {
            slices.push(AtomicSlice::HalfOpen {
                start: Some(window[0]),
                end: Some(window[1]),
            });
        }
        slices.push(AtomicSlice::HalfOpen {
            start: sorted.last().copied(),
            end: None,
        });
    }
    for t in point_ticks {
        slices.push(AtomicSlice::Point(t));
    }
    slices
}

fn run_temporal_graph_checks(
    resolved_edges: &[ResolvedEdge<'_>],
    graphs: &BTreeMap<String, RawLinkGraph>,
    valid_graphs: &HashSet<String>,
    tick_labels: &HashMap<i64, String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (graph_name, raw_graph) in graphs {
        let spec = &raw_graph.spec;
        if !spec.needs_graph_check() || !valid_graphs.contains(graph_name) {
            continue;
        }
        let graph_edges: Vec<ResolvedEdge<'_>> = resolved_edges
            .iter()
            .copied()
            .filter(|e| e.valid_for_graph && e.graph == graph_name)
            .collect();
        if graph_edges.is_empty() {
            continue;
        }

        let slices = build_atomic_slices(&graph_edges);
        let mut reported: HashSet<(&'static str, Vec<usize>)> = HashSet::new();

        for slice in slices {
            let active: Vec<ResolvedEdge<'_>> = graph_edges
                .iter()
                .copied()
                .filter(|e| slice.contains_edge(e.start_tick, e.end_tick))
                .collect();
            if active.is_empty() {
                continue;
            }

            let mut graph: DiGraphMap<&str, usize> = DiGraphMap::new();
            let mut edge_by_idx: HashMap<usize, ResolvedEdge<'_>> = HashMap::new();
            let mut pair_edges: HashMap<(&str, &str), Vec<usize>> = HashMap::new();
            for edge in &active {
                edge_by_idx.insert(edge.idx, *edge);
                pair_edges
                    .entry((edge.from, edge.to))
                    .or_default()
                    .push(edge.idx);
                if graph.edge_weight(edge.from, edge.to).is_none() {
                    graph.add_edge(edge.from, edge.to, edge.idx);
                }
                if !edge.directed && edge.from != edge.to {
                    pair_edges
                        .entry((edge.to, edge.from))
                        .or_default()
                        .push(edge.idx);
                    if graph.edge_weight(edge.to, edge.from).is_none() {
                        graph.add_edge(edge.to, edge.from, edge.idx);
                    }
                }
            }

            let interval = format_slice(slice, tick_labels);

            // 1. acyclic: tarjan_scc + 分量内 DFS 还原具体环路径与边 Span（acyclic 数学上蕴含 asymmetric，跳过重复非对称检查）
            if spec.acyclic {
                for edge in &active {
                    if edge.from == edge.to {
                        let key = ("graph-cycle", vec![edge.idx]);
                        if reported.insert(key) {
                            diagnostics.push(Diagnostic::from_validation(
                                ValidationError::GraphCycle {
                                    graph: graph_name.clone(),
                                    path: format!("{} -> {}", edge.from, edge.to),
                                    interval: interval.clone(),
                                },
                                Some(edge.span),
                            ));
                        }
                    }
                }
                for scc in tarjan_scc(&graph) {
                    if scc.len() <= 1 {
                        continue;
                    }
                    if let Some((cycle_nodes, cycle_edge_ids)) = find_scc_cycle(&graph, &scc) {
                        let mut sig = cycle_edge_ids.clone();
                        sig.sort_unstable();
                        sig.dedup();
                        if reported.insert(("graph-cycle", sig)) {
                            let path = cycle_nodes.join(" -> ");
                            let mut spans: Vec<ByteSpan> = cycle_edge_ids
                                .iter()
                                .filter_map(|id| edge_by_idx.get(id).map(|e| e.span))
                                .collect();
                            spans.sort_by_key(|s| s.start);
                            spans.dedup();
                            let primary = spans.pop();
                            let mut diag = Diagnostic::from_validation(
                                ValidationError::GraphCycle {
                                    graph: graph_name.clone(),
                                    path,
                                    interval: interval.clone(),
                                },
                                primary,
                            );
                            for sp in spans {
                                diag = diag.with_related(sp, "cycle edge here");
                            }
                            diagnostics.push(diag);
                        }
                    }
                }
            } else if spec.asymmetric {
                // 2. asymmetric: 禁止双向互指（含自反边；仅在未启用 acyclic 时独立检查以避免重复诊断）
                for edge in &active {
                    if edge.from == edge.to {
                        let key = ("asymmetry-conflict", vec![edge.idx]);
                        if reported.insert(key) {
                            diagnostics.push(Diagnostic::from_validation(
                                ValidationError::AsymmetryConflict {
                                    graph: graph_name.clone(),
                                    a: edge.from.to_owned(),
                                    b: edge.to.to_owned(),
                                    interval: interval.clone(),
                                },
                                Some(edge.span),
                            ));
                        }
                    }
                }
                for (u, v, &e_uv) in graph.all_edges() {
                    if u < v
                        && let Some(&e_vu) = graph.edge_weight(v, u)
                    {
                        let mut sig = vec![e_uv, e_vu];
                        sig.sort_unstable();
                        if reported.insert(("asymmetry-conflict", sig)) {
                            let first = edge_by_idx[&e_uv];
                            let second = edge_by_idx[&e_vu];
                            let (earlier, later) = if first.span.start <= second.span.start {
                                (first, second)
                            } else {
                                (second, first)
                            };
                            let mut diag = Diagnostic::from_validation(
                                ValidationError::AsymmetryConflict {
                                    graph: graph_name.clone(),
                                    a: u.to_owned(),
                                    b: v.to_owned(),
                                    interval: interval.clone(),
                                },
                                Some(later.span),
                            );
                            if earlier.span != later.span {
                                diag =
                                    diag.with_related(earlier.span, "opposite edge declared here");
                            }
                            diagnostics.push(diag);
                        }
                    }
                }
            }

            // 3. intransitive: 禁止存在三元组 (u, v), (v, w), (u, w)（u, v, w 两两不同）
            if spec.intransitive {
                for (u, v, &e_uv) in graph.all_edges() {
                    if u == v {
                        continue;
                    }
                    for (_, w, &e_vw) in graph.edges_directed(v, Direction::Outgoing) {
                        if w == u || w == v {
                            continue;
                        }
                        if let Some(&e_uw) = graph.edge_weight(u, w) {
                            let mut sig = vec![e_uv, e_vw, e_uw];
                            sig.sort_unstable();
                            if reported.insert(("intransitivity-violation", sig)) {
                                let shortcut = edge_by_idx[&e_uw];
                                let hop1 = edge_by_idx[&e_uv];
                                let hop2 = edge_by_idx[&e_vw];
                                let mut diag = Diagnostic::from_validation(
                                    ValidationError::IntransitivityViolation {
                                        graph: graph_name.clone(),
                                        a: u.to_owned(),
                                        b: v.to_owned(),
                                        c: w.to_owned(),
                                        interval: interval.clone(),
                                    },
                                    Some(shortcut.span),
                                );
                                if hop1.span != shortcut.span {
                                    diag = diag.with_related(
                                        hop1.span,
                                        format!("first hop `{u} -> {v}` here"),
                                    );
                                }
                                if hop2.span != shortcut.span && hop2.span != hop1.span {
                                    diag = diag.with_related(
                                        hop2.span,
                                        format!("second hop `{v} -> {w}` here"),
                                    );
                                }
                                diagnostics.push(diag);
                            }
                        }
                    }
                }
            }

            // 4. functional: 同一源节点指向的不同目标节点数 <= 1
            if spec.functional {
                let mut nodes: Vec<&str> = graph.nodes().collect();
                nodes.sort_unstable();
                for node in nodes {
                    let targets_vec: Vec<&str> =
                        graph.neighbors_directed(node, Direction::Outgoing).collect();
                    if targets_vec.len() <= 1 {
                        continue;
                    }
                    let mut out_edges: Vec<(&str, ResolvedEdge<'_>)> = Vec::new();
                    for target in targets_vec {
                        if let Some(ids) = pair_edges.get(&(node, target)) {
                            for &eid in ids {
                                out_edges.push((target, edge_by_idx[&eid]));
                            }
                        }
                    }
                    out_edges.sort_by_key(|(_, e)| e.span.start);
                    let mut sig: Vec<usize> = out_edges.iter().map(|(_, e)| e.idx).collect();
                    sig.sort_unstable();
                    sig.dedup();
                    if reported.insert(("functional-conflict", sig)) {
                        let mut target_names: Vec<&str> =
                            out_edges.iter().map(|(t, _)| *t).collect();
                        target_names.dedup();
                        let targets = target_names.join(", ");
                        let primary = out_edges.last().map(|(_, e)| e.span);
                        let mut diag = Diagnostic::from_validation(
                            ValidationError::FunctionalConflict {
                                graph: graph_name.clone(),
                                node: node.to_owned(),
                                targets,
                                interval: interval.clone(),
                            },
                            primary,
                        );
                        let mut seen_spans = HashSet::new();
                        if let Some(p) = primary {
                            seen_spans.insert(p);
                        }
                        for (_, earlier) in &out_edges[..out_edges.len().saturating_sub(1)] {
                            if seen_spans.insert(earlier.span) {
                                diag =
                                    diag.with_related(earlier.span, "conflicting outgoing edge here");
                            }
                        }
                        diagnostics.push(diag);
                    }
                }
            }

            // 5. inverse_functional: 同一目标节点接收的不同源节点数 <= 1
            if spec.inverse_functional {
                let mut nodes: Vec<&str> = graph.nodes().collect();
                nodes.sort_unstable();
                for node in nodes {
                    let sources_vec: Vec<&str> =
                        graph.neighbors_directed(node, Direction::Incoming).collect();
                    if sources_vec.len() <= 1 {
                        continue;
                    }
                    let mut in_edges: Vec<(&str, ResolvedEdge<'_>)> = Vec::new();
                    for source in sources_vec {
                        if let Some(ids) = pair_edges.get(&(source, node)) {
                            for &eid in ids {
                                in_edges.push((source, edge_by_idx[&eid]));
                            }
                        }
                    }
                    in_edges.sort_by_key(|(_, e)| e.span.start);
                    let mut sig: Vec<usize> = in_edges.iter().map(|(_, e)| e.idx).collect();
                    sig.sort_unstable();
                    sig.dedup();
                    if reported.insert(("inverse-functional-conflict", sig)) {
                        let mut source_names: Vec<&str> =
                            in_edges.iter().map(|(s, _)| *s).collect();
                        source_names.dedup();
                        let sources = source_names.join(", ");
                        let primary = in_edges.last().map(|(_, e)| e.span);
                        let mut diag = Diagnostic::from_validation(
                            ValidationError::InverseFunctionalConflict {
                                graph: graph_name.clone(),
                                node: node.to_owned(),
                                sources,
                                interval: interval.clone(),
                            },
                            primary,
                        );
                        let mut seen_spans = HashSet::new();
                        if let Some(p) = primary {
                            seen_spans.insert(p);
                        }
                        for (_, earlier) in &in_edges[..in_edges.len().saturating_sub(1)] {
                            if seen_spans.insert(earlier.span) {
                                diag =
                                    diag.with_related(earlier.span, "conflicting incoming edge here");
                            }
                        }
                        diagnostics.push(diag);
                    }
                }
            }
        }
    }
}

/// 在强连通分量（`scc.len() > 1`）内执行简单 DFS，还原出一条具体的有向环路径及其对应边索引。
fn find_scc_cycle<'a>(
    graph: &DiGraphMap<&'a str, usize>,
    scc: &[&'a str],
) -> Option<(Vec<&'a str>, Vec<usize>)> {
    let scc_set: HashSet<&str> = scc.iter().copied().collect();
    let mut sorted_scc: Vec<&str> = scc.to_vec();
    sorted_scc.sort_unstable();

    let mut visited: HashSet<&str> = HashSet::new();
    let mut on_stack: HashMap<&str, usize> = HashMap::new();
    let mut node_stack: Vec<&str> = Vec::new();
    let mut edge_stack: Vec<usize> = Vec::new();

    for &start in &sorted_scc {
        if visited.contains(start) {
            continue;
        }
        if let Some(found) = dfs_cycle(
            graph,
            &scc_set,
            start,
            &mut visited,
            &mut on_stack,
            &mut node_stack,
            &mut edge_stack,
        ) {
            return Some(found);
        }
    }
    None
}

fn dfs_cycle<'a>(
    graph: &DiGraphMap<&'a str, usize>,
    scc_set: &HashSet<&str>,
    curr: &'a str,
    visited: &mut HashSet<&'a str>,
    on_stack: &mut HashMap<&'a str, usize>,
    node_stack: &mut Vec<&'a str>,
    edge_stack: &mut Vec<usize>,
) -> Option<(Vec<&'a str>, Vec<usize>)> {
    visited.insert(curr);
    let pos = node_stack.len();
    on_stack.insert(curr, pos);
    node_stack.push(curr);

    let mut neighbors: Vec<(&'a str, usize)> = graph
        .edges_directed(curr, Direction::Outgoing)
        .filter(|(_, next, _)| scc_set.contains(next))
        .map(|(_, next, &eid)| (next, eid))
        .collect();
    neighbors.sort_unstable_by_key(|(n, _)| *n);

    for (next, eid) in neighbors {
        if let Some(&cycle_start) = on_stack.get(next) {
            let mut cycle_nodes = node_stack[cycle_start..].to_vec();
            cycle_nodes.push(next);
            let mut cycle_edges = edge_stack[cycle_start..].to_vec();
            cycle_edges.push(eid);
            return Some((cycle_nodes, cycle_edges));
        }
        if !visited.contains(next) {
            edge_stack.push(eid);
            if let Some(found) = dfs_cycle(
                graph, scc_set, next, visited, on_stack, node_stack, edge_stack,
            ) {
                return Some(found);
            }
            edge_stack.pop();
        }
    }

    on_stack.remove(curr);
    node_stack.pop();
    None
}

// ---- 时变 duplicate-link 检查 ----

fn intervals_overlap(
    s1: Option<i64>,
    e1: Option<i64>,
    s2: Option<i64>,
    e2: Option<i64>,
) -> bool {
    let p1 = matches!((s1, e1), (Some(a), Some(b)) if a == b);
    let p2 = matches!((s2, e2), (Some(a), Some(b)) if a == b);
    match (p1, p2) {
        (true, true) => s1 == s2,
        (true, false) => {
            let t = s1.unwrap();
            s2.is_none_or(|a| a <= t) && e2.is_none_or(|b| t < b)
        }
        (false, true) => {
            let t = s2.unwrap();
            s1.is_none_or(|a| a <= t) && e1.is_none_or(|b| t < b)
        }
        (false, false) => {
            let start_max = match (s1, s2) {
                (Some(a), Some(b)) => Some(a.max(b)),
                (Some(a), None) | (None, Some(a)) => Some(a),
                (None, None) => None,
            };
            let end_min = match (e1, e2) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (Some(a), None) | (None, Some(a)) => Some(a),
                (None, None) => None,
            };
            match (start_max, end_min) {
                (Some(s), Some(e)) => s < e,
                _ => true,
            }
        }
    }
}

fn lint_duplicate_links_temporal(
    edges: &[ResolvedEdge<'_>],
    lang: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let zh = lang == "zh";
    let rel_msg = if zh {
        "首次声明于此"
    } else {
        "first declared here"
    };
    type DupKey<'a> = (&'a str, bool, &'a str, Option<&'a str>, &'a str);
    type SeenEntry = (Option<i64>, Option<i64>, ByteSpan);
    let mut seen: HashMap<DupKey<'_>, Vec<SeenEntry>> = HashMap::new();

    for edge in edges {
        if !edge.time_valid {
            continue;
        }
        let (norm_left, norm_right) = if edge.directed || edge.from <= edge.to {
            (edge.from, edge.to)
        } else {
            (edge.to, edge.from)
        };
        let key = (edge.graph, edge.directed, norm_left, edge.label, norm_right);
        let bucket = seen.entry(key).or_default();
        if let Some(&(_, _, first_span)) = bucket
            .iter()
            .find(|&&(s, e, _)| intervals_overlap(s, e, edge.start_tick, edge.end_tick))
        {
            let edge_repr = format_edge_repr(edge.from, edge.to, edge.directed, edge.label);
            let msg = if zh {
                format!(
                    "关系图 `{}` 在重叠时段内重复声明了边 `{edge_repr}`",
                    edge.graph
                )
            } else {
                format!(
                    "duplicate link edge `{edge_repr}` with overlapping time interval in `{}`",
                    edge.graph
                )
            };
            let mut diag = Diagnostic::warning("duplicate-link", msg, Some(edge.span));
            if first_span != edge.span {
                diag = diag.with_related(first_span, rel_msg);
            }
            diagnostics.push(diag);
        } else {
            bucket.push((edge.start_tick, edge.end_tick, edge.span));
        }
    }
}

// ---- 等价类（商图）计算（基于 petgraph::unionfind::UnionFind） ----

/// 基于 `equivalence = true` 的实体关系图边计算节点等价类（连通分量），每组内按字典序排列。
pub fn equivalence_classes(
    links: &[Link],
    graphs: &BTreeMap<String, LinkGraph>,
) -> Vec<Vec<String>> {
    let mut id_to_idx: HashMap<&str, usize> = HashMap::new();
    let mut idx_to_id: Vec<&str> = Vec::new();
    let mut eq_pairs: Vec<(usize, usize)> = Vec::new();

    for link in links {
        let Some(graph) = graphs.get(&link.graph) else {
            continue;
        };
        if !graph.equivalence || graph.domain == Some(LinkDomain::Event) {
            continue;
        }
        let a = *id_to_idx.entry(link.from.as_str()).or_insert_with(|| {
            let id = idx_to_id.len();
            idx_to_id.push(link.from.as_str());
            id
        });
        let b = *id_to_idx.entry(link.to.as_str()).or_insert_with(|| {
            let id = idx_to_id.len();
            idx_to_id.push(link.to.as_str());
            id
        });
        eq_pairs.push((a, b));
    }

    if idx_to_id.is_empty() {
        return Vec::new();
    }

    let mut uf = UnionFind::new(idx_to_id.len());
    for (a, b) in eq_pairs {
        uf.union(a, b);
    }

    let mut groups: HashMap<usize, Vec<String>> = HashMap::new();
    for (idx, &node) in idx_to_id.iter().enumerate() {
        let root = uf.find(idx);
        groups.entry(root).or_default().push(node.to_owned());
    }

    let mut result: Vec<Vec<String>> = groups
        .into_values()
        .filter(|g| g.len() > 1)
        .map(|mut g| {
            g.sort_unstable();
            g
        })
        .collect();
    result.sort_by(|a, b| a[0].cmp(&b[0]));
    result
}
