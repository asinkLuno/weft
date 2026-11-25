use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::errors::{ByteSpan, Diagnostic, ValidationError};
use crate::parser::{BinOp, Expr, Span, Spanned, to_byte_span};

pub type TimeUnit = Vec<i64>;
const MAX_PHASE_DEPTH: usize = 64;

/// 运行时时间表达式树：最内层为绝对时间，外层每套一层多一段分量偏移。
#[derive(Debug, Clone, Serialize)]
pub struct Phase {
    pub components: TimeUnit,
    pub anchor: Option<Box<Phase>>,
}

impl Phase {
    fn depth(&self) -> usize {
        match &self.anchor {
            Some(inner) => 1 + inner.depth(),
            None => 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum TimeTarget {
    Anchor(String),
    EntityBaseTime(String),
    EventStart(String),
    EventEnd(String),
}

impl TimeTarget {
    fn display_name(&self) -> String {
        match self {
            Self::Anchor(name) => name.clone(),
            Self::EntityBaseTime(id) => format!("{id}.base_time"),
            Self::EventStart(id) => format!("{id}.start"),
            Self::EventEnd(id) => format!("{id}.end"),
        }
    }

    fn self_event(&self) -> Option<&str> {
        match self {
            Self::EventStart(id) | Self::EventEnd(id) => Some(id.as_str()),
            Self::Anchor(_) | Self::EntityBaseTime(_) => None,
        }
    }
}

/// 解析 `.weft` 中的时间表达式（绝对时间、命名锚点、跨事件/实体引用与 `+`/`-` 偏移），
/// 收集全部循环引用、未知引用与嵌套超限诊断，输出历法无关的 `Phase`。
pub(crate) struct TimeResolver<'a> {
    exprs: &'a HashMap<TimeTarget, Spanned<Expr>>,
    resolved: HashMap<TimeTarget, Option<Phase>>,
    visiting: Vec<(TimeTarget, ByteSpan)>,
    used_anchors: HashSet<String>,
}

impl<'a> TimeResolver<'a> {
    pub(crate) fn new(exprs: &'a HashMap<TimeTarget, Spanned<Expr>>) -> Self {
        Self {
            exprs,
            resolved: HashMap::new(),
            visiting: Vec::new(),
            used_anchors: HashSet::new(),
        }
    }

    pub(crate) fn into_used_anchors(self) -> HashSet<String> {
        self.used_anchors
    }

    pub(crate) fn resolve_target(
        &mut self,
        target: &TimeTarget,
        use_span: Span,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Phase> {
        if let Some(cached) = self.resolved.get(target) {
            return cached.clone();
        }
        let use_byte_span = to_byte_span(use_span);
        if let Some((_, first_span)) = self.visiting.iter().find(|(t, _)| t == target) {
            let mut diag = Diagnostic::from_validation(
                ValidationError::TimeCycle {
                    target: target.display_name(),
                },
                Some(use_byte_span),
            );
            if *first_span != use_byte_span {
                diag = diag.with_related(*first_span, "cycle starts here");
            }
            diagnostics.push(diag);
            return None;
        }
        let Some(expr) = self.exprs.get(target) else {
            diagnostics.push(Diagnostic::from_validation(
                ValidationError::UnknownTimeRef {
                    reference: target.display_name(),
                },
                Some(use_byte_span),
            ));
            return None;
        };
        self.visiting.push((target.clone(), to_byte_span(expr.1)));
        let phase = self.resolve_expr(expr, target.self_event(), diagnostics);
        self.visiting.pop();
        self.resolved.insert(target.clone(), phase.clone());
        phase
    }

    fn resolve_expr(
        &mut self,
        expr: &Spanned<Expr>,
        self_event: Option<&str>,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Phase> {
        let (node, span) = expr;
        let byte_span = to_byte_span(*span);
        let phase = match node {
            Expr::Int(value) => Phase {
                components: vec![*value],
                anchor: None,
            },
            Expr::List(items) => Phase {
                components: self.parse_int_list(items, *span, diagnostics)?,
                anchor: None,
            },
            Expr::Ident(name) => {
                if let Some(event_id) = self_event {
                    if name == "start" {
                        return self.resolve_target(
                            &TimeTarget::EventStart(event_id.to_owned()),
                            *span,
                            diagnostics,
                        );
                    }
                    if name == "end" {
                        return self.resolve_target(
                            &TimeTarget::EventEnd(event_id.to_owned()),
                            *span,
                            diagnostics,
                        );
                    }
                }
                self.used_anchors.insert(name.clone());
                self.resolve_target(&TimeTarget::Anchor(name.clone()), *span, diagnostics)?
            }
            Expr::Member { base, field } => {
                let target = match field.0.as_str() {
                    "start" => TimeTarget::EventStart(base.0.clone()),
                    "end" => TimeTarget::EventEnd(base.0.clone()),
                    "base_time" => TimeTarget::EntityBaseTime(base.0.clone()),
                    _ => {
                        diagnostics.push(Diagnostic::from_validation(
                            ValidationError::UnknownTimeRef {
                                reference: format!("{}.{}", base.0, field.0),
                            },
                            Some(byte_span),
                        ));
                        return None;
                    }
                };
                self.resolve_target(&target, *span, diagnostics)?
            }
            Expr::Binary { lhs, op, rhs } => {
                let anchor = self.resolve_expr(lhs, self_event, diagnostics);
                let offset = self.parse_offset(rhs, diagnostics);
                let (anchor, mut offset) = (anchor?, offset?);
                if *op == BinOp::Sub {
                    for value in &mut offset {
                        match value.checked_neg() {
                            Some(neg) => *value = neg,
                            None => {
                                diagnostics.push(Diagnostic::from_validation(
                                    ValidationError::TimeOffsetOutOfRange,
                                    Some(to_byte_span(rhs.1)),
                                ));
                                return None;
                            }
                        }
                    }
                }
                Phase {
                    components: offset,
                    anchor: Some(Box::new(anchor)),
                }
            }
            _ => {
                diagnostics.push(Diagnostic::from_validation(
                    ValidationError::TimeNonemptyList,
                    Some(byte_span),
                ));
                return None;
            }
        };
        if phase.depth() > MAX_PHASE_DEPTH {
            diagnostics.push(Diagnostic::from_validation(
                ValidationError::TimeNestingTooDeep {
                    maximum: MAX_PHASE_DEPTH.to_string(),
                },
                Some(byte_span),
            ));
            return None;
        }
        Some(phase)
    }

    fn parse_offset(
        &self,
        expr: &Spanned<Expr>,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<TimeUnit> {
        let (node, span) = expr;
        match node {
            Expr::Int(value) => Some(vec![*value]),
            Expr::List(items) => self.parse_int_list(items, *span, diagnostics),
            _ => {
                diagnostics.push(Diagnostic::from_validation(
                    ValidationError::TimeNonemptyList,
                    Some(to_byte_span(*span)),
                ));
                None
            }
        }
    }

    fn parse_int_list(
        &self,
        items: &[Spanned<Expr>],
        list_span: Span,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<TimeUnit> {
        if items.is_empty() {
            diagnostics.push(Diagnostic::from_validation(
                ValidationError::TimeNonemptyList,
                Some(to_byte_span(list_span)),
            ));
            return None;
        }
        let mut values = Vec::with_capacity(items.len());
        let mut ok = true;
        for (item, span) in items {
            match item {
                Expr::Int(value) => values.push(*value),
                _ => {
                    diagnostics.push(Diagnostic::from_validation(
                        ValidationError::TimeIntegerComponent,
                        Some(to_byte_span(*span)),
                    ));
                    ok = false;
                }
            }
        }
        ok.then_some(values)
    }
}
