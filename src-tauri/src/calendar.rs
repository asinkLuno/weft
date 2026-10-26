//! 历法插件：加载与校验 Rune 脚本，执行历算、时间展示和 Story 页序列化。

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use rune::{
    Unit, Vm,
    runtime::{
        GuardedArgs, RuntimeContext, Value as RuneValue, VmError,
    },
};
use serde_json::Value;

use crate::plugin_loader;
use crate::weft::errors::{DocumentError, ValidationError};
use crate::weft::phase::Phase;

pub(crate) const GREGORIAN: &str = "rune/gregorian.rn";

#[derive(Debug)]
pub struct Calendar {
    runtime: Arc<RuntimeContext>,
    unit: Arc<Unit>,
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
        let name = source.display().to_string();
        let unit = Arc::new(
            plugin_loader::compile(&script, &name).map_err(|details| {
                ValidationError::CalendarCompile {
                    calendar: name.clone(),
                    details,
                }
            })?,
        );
        let runtime = plugin_loader::host_runtime();
        let missing = plugin_loader::missing_functions(
            &unit,
            &runtime,
            &[
                "from_tick",
                "to_tick",
                "serialize",
                "humanizer",
                "components",
            ],
        );
        if !missing.is_empty() {
            return Err(ValidationError::CalendarCompile {
                calendar: name.clone(),
                details: format!("缺少接口：{}", missing.join(", ")),
            }
            .into());
        }
        // 分量数取自脚本 components()，加载时调用一次并限定范围。
        let mut vm = Vm::new(runtime.clone(), unit.clone());
        let count = Self::call(&mut vm, &name, "components", ())?
            .as_integer::<i64>()
            .map_err(|_| ValidationError::CalendarInvalidReturn {
                calendar: name.clone(),
                entry: "components".to_owned(),
            })?;
        if !(1..=64).contains(&count) {
            return Err(ValidationError::CalendarComponentsOutOfRange { calendar: name }.into());
        }
        Ok(Self {
            runtime,
            unit,
            source: source.to_path_buf(),
            language: lang.to_owned(),
            components: count as usize,
        })
    }

    // Story 页的历法信息由 serialize 按当前语言生成。
    pub fn serialize(&self, lang: &str) -> Result<Value, DocumentError> {
        let mut vm = self.vm();
        let output = self.call_vm(&mut vm, "serialize", (lang.to_owned(),))?;
        let value = self.script_json("serialize", output)?;
        if !value.is_object() {
            return Err(ValidationError::CalendarInvalidReturn {
                calendar: self.source.display().to_string(),
                entry: "serialize".to_owned(),
            }
            .into());
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
        let mut vm = self.vm();
        let output = self.call_vm(&mut vm, "from_tick", (tick,))?;
        let result: Vec<i64> =
            serde_json::from_value(self.script_json("from_tick", output)?)
                .map_err(|_| self.invalid_return("from_tick"))?;
        if result.len() != self.components {
            return Err(ValidationError::NormalizeWrongComponentCount {
                expected: self.components.to_string(),
            }
            .into());
        }
        Ok(result)
    }

    pub fn humanize(&self, values: &[i64]) -> Result<String, DocumentError> {
        let mut vm = self.vm();
        self.call_vm(
            &mut vm,
            "humanizer",
            (self.pad(values)?, self.language.clone()),
        )?
        .into_string()
        .map(|string| string.into_std())
        .map_err(|_| self.invalid_return("humanizer").into())
    }

    pub fn to_tick(&self, values: &[i64]) -> Result<i64, DocumentError> {
        let mut vm = self.vm();
        self.call_vm(&mut vm, "to_tick", (self.pad(values)?,))?
            .as_integer::<i64>()
            .map_err(|_| self.invalid_return("to_tick").into())
    }

    fn vm(&self) -> Vm {
        Vm::new(self.runtime.clone(), self.unit.clone())
    }

    fn call_vm(
        &self,
        vm: &mut Vm,
        entry: &str,
        args: impl GuardedArgs,
    ) -> Result<RuneValue, DocumentError> {
        Self::call(vm, &self.source.display().to_string(), entry, args)
    }

    fn call(
        vm: &mut Vm,
        calendar: &str,
        entry: &str,
        args: impl GuardedArgs,
    ) -> Result<RuneValue, DocumentError> {
        plugin_loader::call(vm, entry, args).map_err(|error: VmError| {
            ValidationError::CalendarCall {
                calendar: calendar.to_owned(),
                entry: entry.to_owned(),
                details: error.to_string(),
            }
            .into()
        })
    }

    // 脚本返回值经 serde 转成 JSON；无法序列化的值等价于接口返回类型不符。
    fn script_json(&self, entry: &str, value: RuneValue) -> Result<Value, DocumentError> {
        serde_json::to_value(&value).map_err(|_| self.invalid_return(entry).into())
    }

    fn invalid_return(&self, entry: &str) -> ValidationError {
        ValidationError::CalendarInvalidReturn {
            calendar: self.source.display().to_string(),
            entry: entry.to_owned(),
        }
    }

    // 原始 Phase 相减后再归一化，所有历法使用同一条展示流程。
    pub fn offset_display(&self, time: &Phase, base: &Phase) -> Result<String, DocumentError> {
        let difference = time.subtract(base, self.component_count())?;
        self.humanize(&self.normalize(&difference)?)
    }
}
