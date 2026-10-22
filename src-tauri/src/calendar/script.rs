//! 历法脚本加载、接口检查及历算调用；不负责显示语言。

use rhai::serde::from_dynamic;
use rhai::{AST, Dynamic, Engine, Scope};
use serde_json::Value;
use std::{fs, path::Path};

use crate::plugin_loader::{limited_engine, missing_rhai_functions};
use crate::weft::errors::ValidationError;

pub(super) struct CalendarScript {
    engine: Engine,
    ast: AST,
    name: String,
}

impl std::fmt::Debug for CalendarScript {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CalendarScript")
            .field("name", &self.name)
            .finish()
    }
}

impl CalendarScript {
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
        })
    }

    pub(super) fn call(
        &self,
        entry: &str,
        args: impl rhai::FuncArgs,
    ) -> Result<Dynamic, ValidationError> {
        self.engine
            .call_fn(&mut Scope::new(), &self.ast, entry, args)
            .map_err(|error| ValidationError::CalendarCall {
                calendar: self.name.clone(),
                entry: entry.into(),
                details: error.to_string(),
            })
    }

    pub(super) fn invalid_return(&self, entry: &str) -> ValidationError {
        ValidationError::CalendarInvalidReturn {
            calendar: self.name.clone(),
            entry: entry.into(),
        }
    }

    fn ints(&self, values: &[i64]) -> rhai::Array {
        values.iter().copied().map(Dynamic::from_int).collect()
    }

    pub fn metadata(&self) -> Result<Value, ValidationError> {
        from_dynamic(&self.call("metadata", ())?).map_err(|error| {
            ValidationError::CalendarInvalidMetadata {
                calendar: self.name.clone(),
                details: error.to_string(),
            }
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
}
