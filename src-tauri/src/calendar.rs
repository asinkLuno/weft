//! 历法插件：加载与校验 Rhai 脚本，执行历算、时间展示和 Story 页序列化。

use crate::plugin_loader::{limited_engine, missing_rhai_functions};
use crate::weft::errors::{DocumentError, ValidationError};
use crate::weft::phase::Phase;
use rhai::serde::from_dynamic;
use rhai::{AST, Dynamic, Engine, Scope};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(crate) const GREGORIAN: &str = "rhai/gregorian.rhai";

#[derive(Debug)]
pub struct Calendar {
    engine: Engine,
    ast: AST,
    source: PathBuf,
    language: String,
    // 时间分量数，时间列表按它补齐并校验长度。
    components: usize,
}

impl Calendar {
    pub fn new(source: &Path, lang: &str) -> Result<Self, DocumentError> {
        let script =
            fs::read_to_string(source).map_err(|error| ValidationError::CalendarFileRead {
                calendar: source.display().to_string(),
                details: error.to_string(),
            })?;
        let mut engine = limited_engine();
        let language = lang.to_owned();
        engine.register_fn("language", move || language.clone());
        let ast = engine
            .compile(&script)
            .map_err(|error| ValidationError::CalendarCompile {
                calendar: source.display().to_string(),
                details: error.to_string(),
            })?;
        let missing = missing_rhai_functions(
            &ast,
            &[
                ("from_tick", 1),
                ("to_tick", 1),
                ("serialize", 1),
                ("humanizer", 2),
            ],
        );
        if !missing.is_empty() {
            return Err(ValidationError::CalendarCompile {
                calendar: source.display().to_string(),
                details: format!("缺少公开接口：{}", missing.join(", ")),
            }
            .into());
        }
        let components = ast
            .iter_literal_variables(true, false)
            .find(|(name, _, _)| *name == "COMPONENTS")
            .and_then(|(_, _, value)| value.as_int().ok())
            .filter(|count| (1..=64).contains(count))
            .ok_or_else(|| ValidationError::CalendarComponentsOutOfRange {
                calendar: source.display().to_string(),
            })? as usize;
        Ok(Self {
            engine,
            ast,
            source: source.to_path_buf(),
            language: lang.to_owned(),
            components,
        })
    }

    // Story 页的历法信息由 serialize 按当前语言生成。
    pub fn serialize(&self, lang: &str) -> Result<Value, DocumentError> {
        let value: Value = from_dynamic(&self.call("serialize", (lang.to_owned(),))?)
            .map_err(|_| self.invalid_return("serialize"))?;
        if !value.is_object() {
            return Err(self.invalid_return("serialize").into());
        }
        Ok(value)
    }

    pub fn component_count(&self) -> usize {
        self.components
    }

    // 脚本只认 component_count 个分量：超长是写错了，报错；缺尾补零
    fn pad(&self, values: &[i64]) -> Result<Vec<i64>, DocumentError> {
        if values.len() > self.components {
            return Err(ValidationError::TooManyTimeComponents {
                actual: values.len().to_string(),
                maximum: self.components.to_string(),
            }
            .into());
        }
        let mut v = values.to_vec();
        v.resize(self.components, 0);
        Ok(v)
    }

    pub fn normalize(&self, values: &[i64]) -> Result<Vec<i64>, DocumentError> {
        let tick = self.to_tick(values)?;
        let output = self
            .call("from_tick", (tick,))?
            .try_cast::<rhai::Array>()
            .ok_or_else(|| self.invalid_return("from_tick"))?;
        let result: Vec<i64> = output
            .into_iter()
            .map(|item| item.as_int().map_err(|_| self.invalid_return("from_tick")))
            .collect::<Result<_, _>>()?;
        if result.len() != self.components {
            return Err(ValidationError::NormalizeWrongComponentCount {
                expected: self.components.to_string(),
            }
            .into());
        }
        Ok(result)
    }

    pub fn humanize(&self, values: &[i64]) -> Result<String, DocumentError> {
        self.call(
            "humanizer",
            (Self::ints(&self.pad(values)?), self.language.clone()),
        )?
        .into_string()
        .map_err(|_| self.invalid_return("humanizer").into())
    }

    pub fn to_tick(&self, values: &[i64]) -> Result<i64, DocumentError> {
        let padded = self.pad(values)?;
        self.call("to_tick", (Self::ints(&padded),))?
            .as_int()
            .map_err(|_| self.invalid_return("to_tick").into())
    }

    fn call(&self, entry: &str, args: impl rhai::FuncArgs) -> Result<Dynamic, ValidationError> {
        self.engine
            .call_fn(&mut Scope::new(), &self.ast, entry, args)
            .map_err(|error| ValidationError::CalendarCall {
                calendar: self.source.display().to_string(),
                entry: entry.into(),
                details: error.to_string(),
            })
    }

    fn invalid_return(&self, entry: &str) -> ValidationError {
        ValidationError::CalendarInvalidReturn {
            calendar: self.source.display().to_string(),
            entry: entry.into(),
        }
    }

    fn ints(values: &[i64]) -> rhai::Array {
        values.iter().copied().map(Dynamic::from_int).collect()
    }

    // 原始 Phase 相减后再归一化，所有历法使用同一条展示流程。
    pub fn offset_display(&self, time: &Phase, base: &Phase) -> Result<String, DocumentError> {
        let difference = time.subtract(base, self.component_count())?;
        self.humanize(&self.normalize(&difference)?)
    }
}
