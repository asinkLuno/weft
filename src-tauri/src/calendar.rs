use rhai::serde::from_dynamic;
use rhai::{AST, Dynamic, Engine, FnAccess, Scope};
use serde::Serialize;
use serde_json::Value;
use std::{fs, path::Path};

use crate::model::CalendarSpec;
use crate::validation::{DocumentError, ValidationError};

const GREGORIAN: &str = include_str!("../rhai/gregorian.rhai");

pub(crate) fn missing_rhai_functions(ast: &AST, required: &[(&str, usize)]) -> Vec<String> {
    required
        .iter()
        .filter(|(name, arity)| {
            !ast.iter_functions().any(|function| {
                function.name == *name
                    && function.params.len() == *arity
                    && function.access == FnAccess::Public
            })
        })
        .map(|(name, arity)| format!("{name}/{arity}"))
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct CalendarMetadata {
    pub name: String,
    pub title: String,
    pub description: String,
    // 时间分量个数（公历 6：年月日时分秒），时间列表按它补齐/截断校验
    pub components: usize,
}

pub struct RhaiCalendar {
    engine: Engine,
    ast: AST,
    name: String,
    lang: String,
}

impl std::fmt::Debug for RhaiCalendar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RhaiCalendar")
            .field("name", &self.name)
            .finish()
    }
}

impl RhaiCalendar {
    pub fn load(name: &str, path: &Path, lang: &str) -> Result<Self, ValidationError> {
        let script =
            fs::read_to_string(path).map_err(|error| ValidationError::CalendarFileRead {
                calendar: name.into(),
                details: error.to_string(),
            })?;
        Self::load_script(name, &script, lang)
    }

    pub fn load_script(name: &str, script: &str, lang: &str) -> Result<Self, ValidationError> {
        let mut engine = limited_engine();
        let language = lang.to_owned();
        engine.register_fn("language", move || language.clone());
        let ast = engine
            .compile(script)
            .map_err(|error| ValidationError::CalendarCompile {
                calendar: name.into(),
                details: error.to_string(),
            })?;
        let missing = missing_rhai_functions(
            &ast,
            &[
                ("metadata", 0),
                ("normalize", 1),
                ("to_tick", 1),
                ("humanizer", 3),
            ],
        );
        if !missing.is_empty() {
            return Err(ValidationError::CalendarCompile {
                calendar: name.into(),
                details: format!("缺少公开接口：{}", missing.join(", ")),
            });
        }
        Ok(Self {
            engine,
            ast,
            name: name.into(),
            lang: lang.into(),
        })
    }

    fn call(&self, entry: &str, args: impl rhai::FuncArgs) -> Result<Dynamic, ValidationError> {
        self.engine
            .call_fn(&mut Scope::new(), &self.ast, entry, args)
            .map_err(|error| ValidationError::CalendarCall {
                calendar: self.name.clone(),
                entry: entry.into(),
                details: error.to_string(),
            })
    }

    fn invalid_return(&self, entry: &str) -> ValidationError {
        ValidationError::CalendarInvalidReturn {
            calendar: self.name.clone(),
            entry: entry.into(),
        }
    }

    fn ints(&self, values: &[i64]) -> rhai::Array {
        values.iter().copied().map(Dynamic::from_int).collect()
    }

    pub fn metadata(&self) -> Result<Value, ValidationError> {
        let data = self.call("metadata", ())?;
        let output = self.call("humanizer", ("metadata", data, self.lang.clone()))?;
        from_dynamic(&output).map_err(|error| ValidationError::CalendarInvalidMetadata {
            calendar: self.name.clone(),
            details: error.to_string(),
        })
    }

    pub fn normalize(&self, values: &[i64]) -> Result<Vec<i64>, ValidationError> {
        let output = self
            .call("normalize", (self.ints(values),))?
            .try_cast::<rhai::Array>()
            .ok_or_else(|| self.invalid_return("normalize"))?;
        output
            .into_iter()
            .map(|item| item.as_int().map_err(|_| self.invalid_return("normalize")))
            .collect()
    }

    pub fn to_tick(&self, values: &[i64]) -> Result<i64, ValidationError> {
        self.call("to_tick", (self.ints(values),))?
            .as_int()
            .map_err(|_| self.invalid_return("to_tick"))
    }

    pub fn humanize(&self, values: &[i64]) -> Result<String, ValidationError> {
        self.call("humanizer", ("time", self.ints(values), self.lang.clone()))?
            .into_string()
            .map_err(|_| self.invalid_return("humanizer"))
    }

    pub fn humanize_duration(&self, seconds: i64) -> Result<String, ValidationError> {
        self.call("humanizer", ("duration", seconds, self.lang.clone()))?
            .into_string()
            .map_err(|_| self.invalid_return("humanizer"))
    }
}

#[derive(Debug)]
pub struct Calendar {
    inner: RhaiCalendar,
    metadata: CalendarMetadata,
    // 只有内置公历能跨时间点比较：补全缺失分量按最早时刻、时长按 tick 差展示；自定义历法逐分量处理
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
        let inner = if gregorian {
            RhaiCalendar::load_script(name, GREGORIAN, lang)?
        } else {
            let spec = spec.ok_or_else(|| ValidationError::UnregisteredCalendar {
                calendar: name.to_string(),
            })?;
            spec.validate(name, "calendar")?;
            RhaiCalendar::load(name, &base.join(&spec.source), lang)?
        };
        let value = inner.metadata()?;
        let components = value
            .get("components")
            .and_then(Value::as_u64)
            .ok_or_else(|| ValidationError::CalendarComponentsNotPositiveInteger {
                calendar: name.to_string(),
            })?;
        if !(1..=64).contains(&components) {
            return Err(ValidationError::CalendarComponentsOutOfRange {
                calendar: name.to_string(),
            }
            .into());
        }
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
            components: components as usize,
        };
        Ok(Self {
            inner,
            metadata,
            gregorian,
        })
    }

    pub fn metadata(&self) -> CalendarMetadata {
        self.metadata.clone()
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
        let result = self.inner.normalize(&padded)?;
        if result.len() != self.metadata.components {
            return Err(ValidationError::NormalizeWrongComponentCount {
                expected: self.metadata.components.to_string(),
            }
            .into());
        }
        Ok(result)
    }

    pub fn humanize(&self, values: &[i64]) -> Result<String, DocumentError> {
        self.inner.humanize(&self.pad(values)?).map_err(Into::into)
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
        self.inner.to_tick(&padded).map_err(Into::into)
    }

    pub fn offset_display(&self, time: &[i64], base: &[i64]) -> Result<String, DocumentError> {
        if self.gregorian {
            let seconds = self
                .to_tick(time)?
                .checked_sub(self.to_tick(base)?)
                .ok_or(ValidationError::TimeOffsetOutOfRange)?;
            return self.inner.humanize_duration(seconds).map_err(Into::into);
        }
        let diff: Vec<i64> = (0..time.len().max(base.len()))
            .map(|i| {
                time.get(i)
                    .copied()
                    .unwrap_or_default()
                    .checked_sub(base.get(i).copied().unwrap_or_default())
                    .ok_or(ValidationError::TimeOffsetOutOfRange)
            })
            .collect::<Result<_, _>>()?;
        self.humanize(&self.normalize(&diff)?)
    }
}

// 脚本来自用户文件，限制资源防止失控（可计算标签脚本同款，calculable_tag.rs 复用）
pub(crate) fn limited_engine() -> Engine {
    let mut engine = Engine::new();
    engine.set_max_expr_depths(64, 32);
    engine.set_max_call_levels(32);
    engine.set_max_operations(100_000);
    engine.set_max_variables(256);
    engine.set_max_array_size(10_000);
    engine.set_max_map_size(1_000);
    engine.set_max_string_size(1_000_000);
    engine
}
