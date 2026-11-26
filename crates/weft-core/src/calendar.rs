//! 历法插件：加载与校验 Rune 脚本，执行历算、时间展示和 Story 页元信息输出。

use std::{path::Path, sync::Arc};

use rune::{
    Unit, Vm,
    runtime::{GuardedArgs, RuntimeContext, Value as RuneValue, VmError},
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::errors::{DocumentError, ValidationError};
use crate::phase::Phase;
use crate::plugin_loader;

const BUILTIN_GREGORIAN_NAME: &str = "<builtin:gregorian.rn>";
const BUILTIN_GREGORIAN_SRC: &str = include_str!("../rune/gregorian.rn");

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
    source_name: String,
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

/// 供时间轴标尺使用的历法刻度点。
#[derive(Debug, Clone, Serialize)]
pub struct CalendarTick {
    pub tick: i64,
    pub label: String,
}

impl Calendar {
    pub fn builtin_gregorian(lang: &str) -> Result<Self, DocumentError> {
        Self::from_source(BUILTIN_GREGORIAN_SRC, BUILTIN_GREGORIAN_NAME, lang)
    }

    pub fn new(source: &Path, lang: &str) -> Result<Self, DocumentError> {
        let name = source.display().to_string();
        let script = plugin_loader::read_script(source).map_err(|details| {
            ValidationError::CalendarFileRead {
                calendar: name.clone(),
                details,
            }
        })?;
        Self::from_source(&script, &name, lang)
    }

    pub fn from_source(script: &str, name: &str, lang: &str) -> Result<Self, DocumentError> {
        let unit = plugin_loader::compile_cached(script, name).map_err(|details| {
            ValidationError::CalendarCompile {
                calendar: name.to_owned(),
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
                calendar: name.to_owned(),
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
            calendar: name.to_owned(),
            entry: "meta".to_owned(),
            details,
        })?;
        let raw_output = Self::call(&mut vm, name, "meta", (meta_ctx,))?;
        let raw_json = serde_json::to_value(&raw_output).map_err(|_| {
            ValidationError::CalendarInvalidReturn {
                calendar: name.to_owned(),
                entry: "meta".to_owned(),
            }
        })?;
        let raw: RawCalendarMeta = serde_json::from_value(raw_json).map_err(|_| {
            ValidationError::CalendarInvalidReturn {
                calendar: name.to_owned(),
                entry: "meta".to_owned(),
            }
        })?;
        if raw.id.trim().is_empty() || raw.name.trim().is_empty() {
            return Err(ValidationError::CalendarInvalidReturn {
                calendar: name.to_owned(),
                entry: "meta".to_owned(),
            }
            .into());
        }
        if !(1..=64).contains(&raw.components) {
            return Err(ValidationError::CalendarComponentsOutOfRange {
                calendar: name.to_owned(),
            }
            .into());
        }
        let components = raw.components as usize;
        if raw.minimums.len() != components {
            return Err(ValidationError::CalendarInvalidReturn {
                calendar: name.to_owned(),
                entry: "meta".to_owned(),
            }
            .into());
        }

        Ok(Self {
            runtime,
            unit,
            source_name: name.to_owned(),
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
        let mut vm = self.vm();
        self.humanize_with(&mut vm, &moment.values, moment.precision)
    }

    /// 批量将任意 tick 转换为当前历法的展示文案，复用同一 VM。
    pub fn humanize_ticks(&self, ticks: &[i64]) -> Result<Vec<CalendarTick>, DocumentError> {
        let mut vm = self.vm();
        let mut out = Vec::with_capacity(ticks.len());
        for &tick in ticks {
            out.push(self.humanize_tick_with(&mut vm, tick)?);
        }
        Ok(out)
    }

    /// 为时间轴区间 `[start_tick, end_tick]` 生成贴合当前历法边界的刻度点；
    /// 优先按历法分量（如年/月/日/时）步进，无法推进时自动退化为均匀数值刻度。
    pub fn axis_ticks(
        &self,
        start_tick: i64,
        end_tick: i64,
        target_count: usize,
    ) -> Result<Vec<CalendarTick>, DocumentError> {
        if target_count == 0 {
            return Ok(Vec::new());
        }
        let mut vm = self.vm();
        if end_tick <= start_tick {
            return Ok(vec![self.humanize_tick_with(&mut vm, start_tick)?]);
        }

        let span = i128::from(end_tick) - i128::from(start_tick);
        let target = target_count.clamp(2, 24);
        if let Some((level, unit_span)) =
            self.detect_tick_level(&mut vm, start_tick, span, target)?
        {
            let approx_units = (span / unit_span).max(1);
            let stride = nice_stride(approx_units, target as i128);
            let ticks = self.step_calendar_ticks(&mut vm, start_tick, end_tick, level, stride)?;
            if ticks.len() >= 2 {
                return Ok(ticks);
            }
        }

        self.uniform_axis_ticks(&mut vm, start_tick, end_tick, span, target)
    }

    // 探测各分量在区间中点处的大致刻度跨度，选出最粗且步数足够的分量层级。
    fn detect_tick_level(
        &self,
        vm: &mut Vm,
        start_tick: i64,
        span: i128,
        target: usize,
    ) -> Result<Option<(usize, i128)>, DocumentError> {
        let mid_tick = start_tick.saturating_add((span / 2) as i64);
        let mid_vals = self.from_tick_with(vm, mid_tick)?;
        let min_units = i128::from((target / 2).max(2) as u64);

        let mut finest: Option<(usize, i128)> = None;
        for level in 0..self.components {
            let mut base = mid_vals.clone();
            self.truncate_after_level(&mut base, level);
            let mut next = base.clone();
            let Some(incremented) = next[level].checked_add(1) else {
                continue;
            };
            next[level] = incremented;
            let (Ok(t0), Ok(t1)) = (
                self.to_tick_with(vm, &base),
                self.to_tick_with(vm, &next),
            ) else {
                continue;
            };
            let unit_span = i128::from(t1) - i128::from(t0);
            if unit_span <= 0 {
                continue;
            }
            if span / unit_span >= min_units {
                return Ok(Some((level, unit_span)));
            }
            finest = Some((level, unit_span));
        }
        Ok(finest)
    }

    fn step_calendar_ticks(
        &self,
        vm: &mut Vm,
        start_tick: i64,
        end_tick: i64,
        level: usize,
        stride: i64,
    ) -> Result<Vec<CalendarTick>, DocumentError> {
        let mut cur = self.from_tick_with(vm, start_tick)?;
        self.truncate_after_level(&mut cur, level);
        if stride > 1 {
            let origin = self.minimums[level];
            let delta = i128::from(cur[level]) - i128::from(origin);
            let aligned =
                i128::from(origin) + delta.div_euclid(i128::from(stride)) * i128::from(stride);
            if let Ok(v) = i64::try_from(aligned) {
                cur[level] = v;
            }
        }

        let mut out = Vec::new();
        let mut prev_tick: Option<i64> = None;
        for _ in 0..64 {
            let Ok(tick) = self.to_tick_with(vm, &cur) else {
                break;
            };
            if let Some(prev) = prev_tick
                && tick <= prev
            {
                break;
            }
            prev_tick = Some(tick);
            if tick > end_tick {
                break;
            }
            let Ok(mut normalized) = self.from_tick_with(vm, tick) else {
                break;
            };
            if tick >= start_tick {
                let precision = self.effective_precision(&normalized, 1);
                let label = self.humanize_with(vm, &normalized, precision)?;
                out.push(CalendarTick { tick, label });
            }
            self.truncate_after_level(&mut normalized, level);
            let Some(next_val) = normalized[level].checked_add(stride) else {
                break;
            };
            normalized[level] = next_val;
            cur = normalized;
        }
        Ok(out)
    }

    // 退化：区间小于最小历法单位，或脚本不支持分量进位时，按均匀数值刻度生成。
    fn uniform_axis_ticks(
        &self,
        vm: &mut Vm,
        start_tick: i64,
        end_tick: i64,
        span: i128,
        target: usize,
    ) -> Result<Vec<CalendarTick>, DocumentError> {
        let count = target.min((span + 1).min(24) as usize).max(2);
        let mut out = Vec::with_capacity(count);
        let mut last_tick: Option<i64> = None;
        for i in 0..count {
            let tick = if i + 1 == count {
                end_tick
            } else {
                let offset = (span * i128::from(i as u64)) / i128::from((count - 1) as u64);
                i64::try_from(i128::from(start_tick) + offset).unwrap_or(end_tick)
            };
            if last_tick == Some(tick) {
                continue;
            }
            last_tick = Some(tick);
            out.push(self.humanize_tick_with(vm, tick)?);
        }
        Ok(out)
    }

    fn humanize_tick_with(&self, vm: &mut Vm, tick: i64) -> Result<CalendarTick, DocumentError> {
        let values = self.from_tick_with(vm, tick)?;
        let precision = self.effective_precision(&values, 1);
        let label = self.humanize_with(vm, &values, precision)?;
        Ok(CalendarTick { tick, label })
    }

    fn truncate_after_level(&self, values: &mut [i64], level: usize) {
        for j in (level + 1)..self.components {
            values[j] = self.minimums[j];
        }
    }

    // 去掉尾部等于历法起始值的分量，保留最有辨识度的展示精度。
    fn effective_precision(&self, values: &[i64], min_precision: usize) -> usize {
        let last_non_min = values
            .iter()
            .zip(&self.minimums)
            .enumerate()
            .rev()
            .find(|(_, (val, min))| val != min)
            .map_or(0, |(idx, _)| idx);
        (last_non_min + 1)
            .max(min_precision)
            .clamp(1, self.components)
    }

    fn humanize_with(
        &self,
        vm: &mut Vm,
        values: &[i64],
        precision: usize,
    ) -> Result<String, DocumentError> {
        let ctx = plugin_loader::to_rune_value(&json!({
            "api": plugin_loader::API_VERSION,
            "values": values,
            "precision": precision,
            "language": &self.language,
        }))
        .map_err(|details| ValidationError::CalendarCall {
            calendar: self.source_name.clone(),
            entry: "humanizer".to_owned(),
            details,
        })?;
        self.call_vm(vm, "humanizer", (ctx,))?
            .into_string()
            .map(|string| string.into_std())
            .map_err(|_| self.invalid_return("humanizer").into())
    }

    fn from_tick_with(&self, vm: &mut Vm, tick: i64) -> Result<Vec<i64>, DocumentError> {
        let output = self.call_vm(vm, "from_tick", (tick,))?;
        let result: Vec<i64> =
            rune::from_value(output).map_err(|_| self.invalid_return("from_tick"))?;
        if result.len() != self.components {
            return Err(ValidationError::NormalizeWrongComponentCount {
                expected: self.components.to_string(),
            }
            .into());
        }
        Ok(result)
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
        let result = self.from_tick_with(vm, tick)?;
        if self.to_tick_with(vm, &result)? != tick {
            return Err(ValidationError::CalendarRoundTripMismatch {
                calendar: self.source_name.clone(),
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
        Self::call(vm, &self.source_name, entry, args)
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
            calendar: self.source_name.clone(),
            entry: entry.to_owned(),
        }
    }
}

// 1-2-5 系列步长，使刻度总数贴近 target_count。
fn nice_stride(approx_units: i128, target_count: i128) -> i64 {
    let target = target_count.max(1);
    let raw = (approx_units + target - 1) / target;
    if raw <= 1 {
        return 1;
    }
    let mut mag: i128 = 1;
    while mag.saturating_mul(10) <= raw {
        mag = mag.saturating_mul(10);
    }
    for mult in [1, 2, 5, 10] {
        let candidate = mag.saturating_mul(mult);
        if candidate >= raw {
            return i64::try_from(candidate).unwrap_or(i64::MAX);
        }
    }
    i64::try_from(raw).unwrap_or(i64::MAX)
}
