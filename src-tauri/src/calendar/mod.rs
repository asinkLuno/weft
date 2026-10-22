//! 历法应用入口：分量校验、历算与展示的编排。
//! phase 管时间结构，script 管 Rhai 历算接口，humanizer 管本地化。

mod humanizer;
mod script;

use self::{humanizer::Humanizer, script::CalendarScript};
use crate::weft::errors::{DocumentError, ValidationError};
use crate::weft::phase::Phase;
use crate::weft::plugin::CalendarSpec;
use serde::Serialize;
use serde_json::Value;
use std::path::Path;

const GREGORIAN: &str = include_str!("../../rhai/gregorian.rhai");

#[derive(Debug, Clone, Serialize)]
pub struct CalendarMetadata {
    pub name: String,
    pub title: String,
    pub description: String,
    // 时间分量个数（公历 6：年月日时分秒），时间列表按它补齐并校验长度
    pub components: usize,
}

#[derive(Debug)]
pub struct Calendar {
    script: CalendarScript,
    humanizer: Humanizer,
    metadata: CalendarMetadata,
    // 内置公历的特殊规则：排序时将缺失的月、日补为最早时刻
    gregorian: bool,
}

impl Calendar {
    pub fn load(
        name: &str,
        spec: Option<&CalendarSpec>,
        base: &Path,
        lang: &str,
    ) -> Result<Self, DocumentError> {
        let gregorian = name == "gregorian";
        let script = if gregorian {
            CalendarScript::load_script(name, GREGORIAN, lang)?
        } else {
            let spec = spec.ok_or_else(|| ValidationError::UnregisteredCalendar {
                calendar: name.to_string(),
            })?;
            spec.validate(name, "calendar")?;
            CalendarScript::load(name, &base.join(&spec.source), lang)?
        };
        let raw_metadata = script.metadata()?;
        let value = &raw_metadata;
        let components = value
            .get("components")
            .and_then(Value::as_u64)
            .filter(|count| *count > 0)
            .ok_or_else(|| ValidationError::CalendarComponentsNotPositiveInteger {
                calendar: name.to_string(),
            })?;
        let components = usize::try_from(components).map_err(|_| {
            ValidationError::CalendarComponentsOutOfRange {
                calendar: name.to_string(),
            }
        })?;
        let humanizer = Humanizer::new(lang);
        let value = humanizer.metadata(&script, &raw_metadata)?;
        let metadata = CalendarMetadata {
            name: name.into(),
            title: value
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or(name)
                .into(),
            description: value
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .into(),
            components,
        };
        Ok(Self {
            script,
            humanizer,
            metadata,
            gregorian,
        })
    }

    pub fn metadata(&self) -> CalendarMetadata {
        self.metadata.clone()
    }

    pub fn component_count(&self) -> usize {
        self.metadata.components
    }

    // 脚本只认 component_count 个分量：超长是写错了，报错；缺尾补零
    fn pad(&self, values: &[i64]) -> Result<Vec<i64>, DocumentError> {
        if values.len() > self.metadata.components {
            return Err(ValidationError::TooManyTimeComponents {
                actual: values.len().to_string(),
                maximum: self.metadata.components.to_string(),
            }
            .into());
        }
        let mut v = values.to_vec();
        v.resize(self.metadata.components, 0);
        Ok(v)
    }

    pub fn normalize(&self, values: &[i64]) -> Result<Vec<i64>, DocumentError> {
        let padded = self.pad(values)?;
        let result = self.script.normalize(&padded)?;
        if result.len() != self.metadata.components {
            return Err(ValidationError::NormalizeWrongComponentCount {
                expected: self.metadata.components.to_string(),
            }
            .into());
        }
        Ok(result)
    }

    pub fn humanize(&self, values: &[i64]) -> Result<String, DocumentError> {
        self.humanizer
            .time(&self.script, &self.pad(values)?)
            .map_err(Into::into)
    }

    pub fn to_tick(&self, values: &[i64]) -> Result<i64, DocumentError> {
        let mut padded = self.pad(values)?;
        if self.gregorian {
            // 部分日期排序按其最早可能时刻；展示仍保留未指定的 0 分量。
            if padded[1] == 0 {
                padded[1] = 1;
            }
            if padded[2] == 0 {
                padded[2] = 1;
            }
        }
        self.script.to_tick(&padded).map_err(Into::into)
    }

    // 原始 Phase 相减后再归一化，所有历法使用同一条展示流程。
    pub fn offset_display(&self, time: &Phase, base: &Phase) -> Result<String, DocumentError> {
        let difference = time.subtract(base, self.component_count())?;
        self.humanize(&self.normalize(&difference)?)
    }
}
