use serde::Serialize;
use serde_yaml_ng::Value as Yaml;

use crate::weft::errors::{DocumentError, ValidationError};

// 时间分量按历法从大到小排列；展平后的长度严格取自 Rhai metadata().components。
// 输入可省略末尾分量，展平时补零；进位交给历法处理。
pub type TimeUnit = Vec<i64>;

// phase 可以是 time_unit，也可以是 [phase, 偏移分量…]，递归锚点位于首项。
// [[2025, 4, 2, 10], 0, 0, 0, 2] 展平为 [2025, 4, 2, 12]。
// 负数分量表示减法；flatten 只逐分量求和，不处理进位或历法规则。
#[derive(Debug, Clone, Serialize)]
pub struct Phase {
    pub components: TimeUnit,
    pub anchor: Option<Box<Phase>>,
}

impl Phase {
    pub(crate) fn from_yaml(value: &Yaml) -> Result<Self, DocumentError> {
        let sequence = value
            .as_sequence()
            .ok_or_else(|| DocumentError::validation(ValidationError::TimeNonemptyList))?;
        if sequence.is_empty() {
            return Err(ValidationError::TimeNonemptyList.into());
        }
        let (components, anchor) = match sequence.first() {
            Some(Yaml::Sequence(_)) => (&sequence[1..], sequence.first()),
            _ => (&sequence[..], None),
        };
        let mut base = Vec::with_capacity(components.len());
        for component in components {
            base.push(
                component.as_i64().ok_or_else(|| {
                    DocumentError::validation(ValidationError::TimeIntegerComponent)
                })?,
            );
        }
        let anchor = anchor.map(Self::from_yaml).transpose()?.map(Box::new);
        Ok(Self {
            components: base,
            anchor,
        })
    }

    // 从最内层锚点开始递归展平，每层偏移都从第一个分量开始相加。
    // 六分量历法下，[[[2025, 4, 2, 10], 0, 0, 0, 2], 1, 2, 3]
    // 得到 [2026, 6, 5, 12, 0, 0]；进位由 Rhai normalize 处理。
    pub(crate) fn flatten(&self, component_count: usize) -> Result<TimeUnit, DocumentError> {
        let mut result = vec![0; component_count];
        self.accumulate(&mut result)?;
        Ok(result)
    }

    /// 两个 Phase 按相同位数展平后，计算 self - other；不处理历法进位或借位。
    pub fn subtract(
        &self,
        other: &Self,
        component_count: usize,
    ) -> Result<TimeUnit, DocumentError> {
        let mut difference = self.flatten(component_count)?;
        let other = other.flatten(component_count)?;
        for (target, value) in difference.iter_mut().zip(other) {
            *target = target
                .checked_sub(value)
                .ok_or(ValidationError::TimeOffsetOutOfRange)?;
        }
        Ok(difference)
    }

    // 各层共用同一个结果缓冲区，先累加锚点，再累加当前层的分量。
    fn accumulate(&self, result: &mut [i64]) -> Result<(), DocumentError> {
        if self.components.len() > result.len() {
            return Err(ValidationError::TooManyTimeComponents {
                actual: self.components.len().to_string(),
                maximum: result.len().to_string(),
            }
            .into());
        }
        if let Some(anchor) = &self.anchor {
            anchor.accumulate(result)?;
        }
        for (target, value) in result.iter_mut().zip(&self.components) {
            *target = target
                .checked_add(*value)
                .ok_or(ValidationError::TimeOffsetOutOfRange)?;
        }
        Ok(())
    }
}
