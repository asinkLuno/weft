//! 显示适配层：集中管理 humanizer 的种类、语言和返回值转换。

use rhai::{
    Dynamic,
    serde::{from_dynamic, to_dynamic},
};
use serde_json::Value;

use super::script::CalendarScript;
use crate::weft::errors::ValidationError;

#[derive(Debug)]
pub(super) struct Humanizer {
    language: String,
}

impl Humanizer {
    pub fn new(language: &str) -> Self {
        Self {
            language: language.into(),
        }
    }

    fn call(
        &self,
        script: &CalendarScript,
        kind: &str,
        value: Dynamic,
    ) -> Result<Dynamic, ValidationError> {
        script.call("humanizer", (kind.to_owned(), value, self.language.clone()))
    }

    pub fn metadata(
        &self,
        script: &CalendarScript,
        value: &Value,
    ) -> Result<Value, ValidationError> {
        let value = to_dynamic(value).map_err(|_| script.invalid_return("metadata"))?;
        from_dynamic(&self.call(script, "metadata", value)?)
            .map_err(|_| script.invalid_return("humanizer"))
    }

    pub fn time(&self, script: &CalendarScript, values: &[i64]) -> Result<String, ValidationError> {
        let values: rhai::Array = values.iter().copied().map(Dynamic::from_int).collect();
        self.call(script, "time", Dynamic::from_array(values))?
            .into_string()
            .map_err(|_| script.invalid_return("humanizer"))
    }
}
