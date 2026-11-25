use std::collections::HashMap;

use serde::Serialize;

use crate::weft::errors::{DocumentError, ValidationError};
use crate::weft::parser::{BinOp, Expr, Span, Spanned, source_span};

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
/// 检测循环引用与嵌套超限，输出历法无关的 `Phase`。
pub(crate) struct TimeResolver<'a> {
    src: &'a str,
    exprs: &'a HashMap<TimeTarget, Spanned<Expr>>,
    resolved: HashMap<TimeTarget, Phase>,
    visiting: Vec<TimeTarget>,
}

impl<'a> TimeResolver<'a> {
    pub(crate) fn new(src: &'a str, exprs: &'a HashMap<TimeTarget, Spanned<Expr>>) -> Self {
        Self {
            src,
            exprs,
            resolved: HashMap::new(),
            visiting: Vec::new(),
        }
    }

    pub(crate) fn resolve_target(
        &mut self,
        target: &TimeTarget,
        use_span: Span,
    ) -> Result<Phase, DocumentError> {
        if let Some(cached) = self.resolved.get(target) {
            return Ok(cached.clone());
        }
        if self.visiting.contains(target) {
            return Err(DocumentError::validation_at(
                ValidationError::TimeCycle {
                    target: target.display_name(),
                },
                source_span(self.src, use_span),
            ));
        }
        let Some(expr) = self.exprs.get(target) else {
            return Err(DocumentError::validation_at(
                ValidationError::UnknownTimeRef {
                    reference: target.display_name(),
                },
                source_span(self.src, use_span),
            ));
        };
        self.visiting.push(target.clone());
        let result = self.resolve_expr(expr, target.self_event());
        self.visiting.pop();
        let phase = result?;
        self.resolved.insert(target.clone(), phase.clone());
        Ok(phase)
    }

    fn resolve_expr(
        &mut self,
        expr: &Spanned<Expr>,
        self_event: Option<&str>,
    ) -> Result<Phase, DocumentError> {
        let (node, span) = expr;
        let phase = match node {
            Expr::Int(value) => Phase {
                components: vec![*value],
                anchor: None,
            },
            Expr::List(items) => Phase {
                components: self.parse_int_list(items, *span)?,
                anchor: None,
            },
            Expr::Ident(name) => {
                if let Some(event_id) = self_event {
                    if name == "start" {
                        return self.resolve_target(
                            &TimeTarget::EventStart(event_id.to_owned()),
                            *span,
                        );
                    }
                    if name == "end" {
                        return self
                            .resolve_target(&TimeTarget::EventEnd(event_id.to_owned()), *span);
                    }
                }
                self.resolve_target(&TimeTarget::Anchor(name.clone()), *span)?
            }
            Expr::Member { base, field } => {
                let target = match field.0.as_str() {
                    "start" => TimeTarget::EventStart(base.0.clone()),
                    "end" => TimeTarget::EventEnd(base.0.clone()),
                    "base_time" => TimeTarget::EntityBaseTime(base.0.clone()),
                    _ => {
                        return Err(DocumentError::validation_at(
                            ValidationError::UnknownTimeRef {
                                reference: format!("{}.{}", base.0, field.0),
                            },
                            source_span(self.src, *span),
                        ));
                    }
                };
                self.resolve_target(&target, *span)?
            }
            Expr::Binary { lhs, op, rhs } => {
                let anchor = self.resolve_expr(lhs, self_event)?;
                let mut offset = self.parse_offset(rhs)?;
                if *op == BinOp::Sub {
                    for value in &mut offset {
                        *value = value.checked_neg().ok_or_else(|| {
                            DocumentError::validation_at(
                                ValidationError::TimeOffsetOutOfRange,
                                source_span(self.src, rhs.1),
                            )
                        })?;
                    }
                }
                Phase {
                    components: offset,
                    anchor: Some(Box::new(anchor)),
                }
            }
            _ => {
                return Err(DocumentError::validation_at(
                    ValidationError::TimeNonemptyList,
                    source_span(self.src, *span),
                ));
            }
        };
        if phase.depth() > MAX_PHASE_DEPTH {
            return Err(DocumentError::validation_at(
                ValidationError::TimeNestingTooDeep {
                    maximum: MAX_PHASE_DEPTH.to_string(),
                },
                source_span(self.src, *span),
            ));
        }
        Ok(phase)
    }

    fn parse_offset(&self, expr: &Spanned<Expr>) -> Result<TimeUnit, DocumentError> {
        let (node, span) = expr;
        match node {
            Expr::Int(value) => Ok(vec![*value]),
            Expr::List(items) => self.parse_int_list(items, *span),
            _ => Err(DocumentError::validation_at(
                ValidationError::TimeNonemptyList,
                source_span(self.src, *span),
            )),
        }
    }

    fn parse_int_list(
        &self,
        items: &[Spanned<Expr>],
        list_span: Span,
    ) -> Result<TimeUnit, DocumentError> {
        if items.is_empty() {
            return Err(DocumentError::validation_at(
                ValidationError::TimeNonemptyList,
                source_span(self.src, list_span),
            ));
        }
        items
            .iter()
            .map(|(item, span)| match item {
                Expr::Int(value) => Ok(*value),
                _ => Err(DocumentError::validation_at(
                    ValidationError::TimeIntegerComponent,
                    source_span(self.src, *span),
                )),
            })
            .collect()
    }
}
