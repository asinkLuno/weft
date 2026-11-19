use serde::Serialize;
use serde_yaml_ng::Value as Yaml;

use crate::weft::errors::{DocumentError, ValidationError};


// YAML 里 phase 可以是 time_unit，也可以是 [phase, 偏移分量…]，递归锚点位于首项。
// 解析时就把递归摊平：最内层是绝对时间，外面每套一层多一段偏移。
// [[[2025, 4, 2, 10], 0, 0, 0, 2], 1] 得到 base = [2025, 4, 2, 10]，
// offsets = [[0, 0, 0, 2], [1]]。

pub type TimeUnit = Vec<i64>;
const MAX_PHASE_DEPTH: usize = 64;
// phase 可以是 time_unit，也可以是 [phase, 偏移分量…]；锚点递归深度最多 64 层。
#[derive(Debug, Clone, Serialize)]
pub struct Phase {
    pub components: TimeUnit,
    pub anchor: Option<Box<Phase>>,
}

impl Phase {
    pub(crate) fn from_yaml(value: &Yaml) -> Result<Self, DocumentError> {
        Self::from_yaml_at_depth(value, 1)
    }

    fn from_yaml_at_depth(value: &Yaml, depth: usize) -> Result<Self, DocumentError> {
        if depth > MAX_PHASE_DEPTH {
            return Err(ValidationError::TimeNestingTooDeep {
                maximum: MAX_PHASE_DEPTH.to_string(),
            }
            .into());
        }
        let sequence = value
            .as_sequence()
            .ok_or_else(|| DocumentError::validation(ValidationError::TimeNonemptyList))?;
        match sequence.first() {
            None => Err(ValidationError::TimeNonemptyList.into()),
            Some(anchor @ Yaml::Sequence(_)) => Ok(Self {
                components: integers(&sequence[1..])?,
                anchor: Some(Box::new(Self::from_yaml_at_depth(anchor, depth + 1)?)),
            }),
            Some(_) => Ok(Self {
                components: integers(sequence)?,
                anchor: None,
            }),
        }
    }


    /// 标量兼容：单个整数等价于单分量 Phase，其余结构交给 [`Phase::from_yaml`]。
    /// 实体的 `base_time` 和事件的 `start_time` / `end_time` 共用。
    pub(crate) fn from_yaml_scalar(value: &Yaml) -> Result<Self, DocumentError> {
        match value {
            Yaml::Number(_) => Self::from_yaml(&Yaml::Sequence(vec![value.clone()])),
            value => Self::from_yaml(value),
        }
    }

    /// 标量形式再兼容缺省：Null 或未提供都视为 None。
    pub(crate) fn from_yaml_scalar_opt(
        value: Option<&Yaml>,
    ) -> Result<Option<Self>, DocumentError> {
        match value {
            None | Some(Yaml::Null) => Ok(None),
            Some(value) => Self::from_yaml_scalar(value).map(Some),
        }
    }
}

fn integers(values: &[Yaml]) -> Result<TimeUnit, DocumentError> {
    values
        .iter()
        .map(|value| {
            value
                .as_i64()
                .ok_or_else(|| DocumentError::validation(ValidationError::TimeIntegerComponent))
        })
        .collect()
}
