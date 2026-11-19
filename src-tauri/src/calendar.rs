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
    // 时间分量数，时间列表按它校验长度。
    components: usize,
    // 各分量的起始值，用来补齐绝对时间里没写的尾部分量；长度等于 components。
    minimums: Vec<i64>,
}

/// 解析完成的时间点。
#[derive(Debug, Clone)]
pub struct Moment {
    /// 归一化后的合法日期，长度等于历法分量数。
    pub values: Vec<i64>,
    /// 排序与起止比较用的刻度；模糊时间取该时段的起点。
    pub tick: i64,
    /// 作者写到第几个分量，展示时只显示这么多。
    pub precision: usize,
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
                "minimums",
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
        let components = count as usize;
        // 起始值同样只取一次；长度不符等价于返回值类型不符。
        let mut vm = Vm::new(runtime.clone(), unit.clone());
        let minimums = serde_json::to_value(Self::call(&mut vm, &name, "minimums", ())?)
            .ok()
            .and_then(|value| serde_json::from_value::<Vec<i64>>(value).ok())
            .filter(|minimums| minimums.len() == components)
            .ok_or_else(|| ValidationError::CalendarInvalidReturn {
                calendar: name.clone(),
                entry: "minimums".to_owned(),
            })?;
        Ok(Self {
            runtime,
            unit,
            source: source.to_path_buf(),
            language: lang.to_owned(),
            components,
            minimums,
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

    // 从递归锚点取得绝对时间，再按由内到外的顺序应用每层偏移。
    pub fn resolve(&self, phase: &Phase) -> Result<Moment, DocumentError> {
        let mut chain = Vec::new();
        let mut current = phase;
        loop {
            chain.push(current);
            match current.anchor.as_deref() {
                Some(anchor) => current = anchor,
                None => break,
            }
        }
        let base = chain
            .last()
            .expect("phase chain always contains its root")
            .components
            .as_slice();
        let (mut values, mut tick) = self.normalize(&self.fill(base)?)?;
        let mut precision = base.len();
        for layer in chain.iter().rev().skip(1) {
            let offset = &layer.components;
            self.check_length(offset)?;
            for (target, delta) in values.iter_mut().zip(offset) {
                *target = target
                    .checked_add(*delta)
                    .ok_or(ValidationError::TimeOffsetOutOfRange)?;
            }
            (values, tick) = self.normalize(&values)?;
            precision = precision.max(offset.len());
        }
        Ok(Moment {
            values,
            tick,
            precision,
        })
    }

    pub fn humanize(&self, moment: &Moment) -> Result<String, DocumentError> {
        let mut vm = self.vm();
        self.call_vm(
            &mut vm,
            "humanizer",
            (
                moment.values.clone(),
                moment.precision as i64,
                self.language.clone(),
            ),
        )?
        .into_string()
        .map(|string| string.into_std())
        .map_err(|_| self.invalid_return("humanizer").into())
    }

    // 脚本只认 components 个分量，超长是写错了。
    fn check_length(&self, values: &[i64]) -> Result<(), DocumentError> {
        if values.len() > self.components {
            return Err(ValidationError::TooManyTimeComponents {
                actual: values.len().to_string(),
                maximum: self.components.to_string(),
            }
            .into());
        }
        Ok(())
    }

    // 绝对时间没写的尾部分量取历法起始值（公历的月、日是 1，不是 0）。
    // 写出来的分量低于起始值是笔误或把偏移写成了绝对时间，在入口拒绝；
    // 首项是最高位，没有下界。
    fn fill(&self, base: &[i64]) -> Result<Vec<i64>, DocumentError> {
        self.check_length(base)?;
        let mut values = self.minimums.clone();
        for (position, (target, value)) in values.iter_mut().zip(base).enumerate() {
            if position > 0 && value < target {
                return Err(ValidationError::TimeComponentBelowMinimum {
                    position: (position + 1).to_string(),
                    value: value.to_string(),
                    minimum: target.to_string(),
                }
                .into());
            }
            *target = *value;
        }
        Ok(values)
    }

    // normalize(values) = from_tick(to_tick(values))，并核对归一化结果能换回同一刻度：
    // 两个函数对不上时日期和排序会各说各话，在这里拦下。
    fn normalize(&self, values: &[i64]) -> Result<(Vec<i64>, i64), DocumentError> {
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
        if self.to_tick(&result)? != tick {
            return Err(ValidationError::CalendarRoundTripMismatch {
                calendar: self.source.display().to_string(),
            }
            .into());
        }
        Ok((result, tick))
    }

    fn to_tick(&self, values: &[i64]) -> Result<i64, DocumentError> {
        let mut vm = self.vm();
        self.call_vm(&mut vm, "to_tick", (values.to_vec(),))?
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
}
