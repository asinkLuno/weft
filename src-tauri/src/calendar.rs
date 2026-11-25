//! 历法插件：加载与校验 Rune 脚本，执行历算、时间展示和 Story 页元信息输出。

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use rune::{
    Unit, Vm,
    runtime::{GuardedArgs, RuntimeContext, Value as RuneValue, VmError},
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::plugin_loader;
use crate::weft::errors::{DocumentError, ValidationError};
use crate::weft::phase::Phase;

pub(crate) const GREGORIAN: &str = "rune/gregorian.rn";

/// 历法脚本 `meta(ctx)` 输出的公开元信息。
#[derive(Debug, Clone, Serialize)]
pub struct CalendarMeta {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Deserialize)]
struct RawCalendarMeta {
    id: String,
    name: String,
    #[serde(default)]
    description: Option<String>,
    components: i64,
    minimums: Vec<i64>,
}

#[derive(Debug)]
pub struct Calendar {
    runtime: Arc<RuntimeContext>,
    unit: Arc<Unit>,
    source: PathBuf,
    language: String,
    meta: CalendarMeta,
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
        let name = source.display().to_string();
        let script = plugin_loader::read_script(source).map_err(|details| {
            ValidationError::CalendarFileRead {
                calendar: name.clone(),
                details,
            }
        })?;
        let unit = plugin_loader::compile_cached(&script, &name).map_err(|details| {
            ValidationError::CalendarCompile {
                calendar: name.clone(),
                details,
            }
        })?;
        let runtime = plugin_loader::host_runtime();
        let missing = plugin_loader::missing_functions(
            &unit,
            &runtime,
            &["meta", "from_tick", "to_tick", "humanizer"],
        );
        if !missing.is_empty() {
            return Err(ValidationError::CalendarMissingInterface {
                calendar: name,
                functions: missing.join(", "),
            }
            .into());
        }

        // 加载期调用一次 meta(ctx)，一次性取得稳定 id、本地化展示文案、分量数与各分量起始值。
        let mut vm = Vm::new(Arc::clone(&runtime), Arc::clone(&unit));
        let meta_ctx = plugin_loader::to_rune_value(&json!({
            "api": plugin_loader::API_VERSION,
            "language": lang,
        }))
        .map_err(|details| ValidationError::CalendarCall {
            calendar: name.clone(),
            entry: "meta".to_owned(),
            details,
        })?;
        let raw_output = Self::call(&mut vm, &name, "meta", (meta_ctx,))?;
        let raw_json = serde_json::to_value(&raw_output).map_err(|_| {
            ValidationError::CalendarInvalidReturn {
                calendar: name.clone(),
                entry: "meta".to_owned(),
            }
        })?;
        let raw: RawCalendarMeta = serde_json::from_value(raw_json).map_err(|_| {
            ValidationError::CalendarInvalidReturn {
                calendar: name.clone(),
                entry: "meta".to_owned(),
            }
        })?;
        if raw.id.trim().is_empty() || raw.name.trim().is_empty() {
            return Err(ValidationError::CalendarInvalidReturn {
                calendar: name,
                entry: "meta".to_owned(),
            }
            .into());
        }
        if !(1..=64).contains(&raw.components) {
            return Err(ValidationError::CalendarComponentsOutOfRange { calendar: name }.into());
        }
        let components = raw.components as usize;
        if raw.minimums.len() != components {
            return Err(ValidationError::CalendarInvalidReturn {
                calendar: name,
                entry: "meta".to_owned(),
            }
            .into());
        }

        Ok(Self {
            runtime,
            unit,
            source: source.to_path_buf(),
            language: lang.to_owned(),
            meta: CalendarMeta {
                id: raw.id,
                name: raw.name,
                description: raw.description,
            },
            components,
            minimums: raw.minimums,
        })
    }

    pub fn id(&self) -> &str {
        &self.meta.id
    }

    pub fn meta(&self) -> &CalendarMeta {
        &self.meta
    }

    pub fn component_count(&self) -> usize {
        self.components
    }

    pub fn minimums(&self) -> &[i64] {
        &self.minimums
    }

    // 从递归锚点取得绝对时间，再按由内到外的顺序应用每层偏移；整条链复用同一个 VM。
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
        let mut vm = self.vm();
        let (mut values, mut tick) = self.normalize_with(&mut vm, &self.fill(base)?)?;
        let mut precision = base.len();
        for layer in chain.iter().rev().skip(1) {
            let offset = &layer.components;
            self.check_length(offset)?;
            for (target, delta) in values.iter_mut().zip(offset) {
                *target = target
                    .checked_add(*delta)
                    .ok_or(ValidationError::TimeOffsetOutOfRange)?;
            }
            (values, tick) = self.normalize_with(&mut vm, &values)?;
            precision = precision.max(offset.len());
        }
        Ok(Moment {
            values,
            tick,
            precision,
        })
    }

    pub fn humanize(&self, moment: &Moment) -> Result<String, DocumentError> {
        let ctx = plugin_loader::to_rune_value(&json!({
            "api": plugin_loader::API_VERSION,
            "values": &moment.values,
            "precision": moment.precision,
            "language": &self.language,
        }))
        .map_err(|details| ValidationError::CalendarCall {
            calendar: self.source.display().to_string(),
            entry: "humanizer".to_owned(),
            details,
        })?;
        let mut vm = self.vm();
        self.call_vm(&mut vm, "humanizer", (ctx,))?
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
    fn normalize_with(
        &self,
        vm: &mut Vm,
        values: &[i64],
    ) -> Result<(Vec<i64>, i64), DocumentError> {
        let tick = self.to_tick_with(vm, values)?;
        let output = self.call_vm(vm, "from_tick", (tick,))?;
        let result: Vec<i64> =
            rune::from_value(output).map_err(|_| self.invalid_return("from_tick"))?;
        if result.len() != self.components {
            return Err(ValidationError::NormalizeWrongComponentCount {
                expected: self.components.to_string(),
            }
            .into());
        }
        if self.to_tick_with(vm, &result)? != tick {
            return Err(ValidationError::CalendarRoundTripMismatch {
                calendar: self.source.display().to_string(),
            }
            .into());
        }
        Ok((result, tick))
    }

    fn to_tick_with(&self, vm: &mut Vm, values: &[i64]) -> Result<i64, DocumentError> {
        self.call_vm(vm, "to_tick", (values.to_vec(),))?
            .as_integer::<i64>()
            .map_err(|_| self.invalid_return("to_tick").into())
    }

    fn vm(&self) -> Vm {
        Vm::new(Arc::clone(&self.runtime), Arc::clone(&self.unit))
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

    fn invalid_return(&self, entry: &str) -> ValidationError {
        ValidationError::CalendarInvalidReturn {
            calendar: self.source.display().to_string(),
            entry: entry.to_owned(),
        }
    }
}
