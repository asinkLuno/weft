use rhai::serde::from_dynamic;
use rhai::{Dynamic, Engine, Scope, AST};
use serde::Serialize;
use serde_json::Value;
use std::{fs, path::Path};

use crate::model::CalendarSpec;

const GREGORIAN_CORE: &str = include_str!("../rhai/gregorian_core.rhai");
const GREGORIAN_CN: &str = include_str!("../rhai/gregorian_cn.rhai");
const GREGORIAN_EN: &str = include_str!("../rhai/gregorian_en.rhai");

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
}

impl std::fmt::Debug for RhaiCalendar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RhaiCalendar").field("name", &self.name).finish()
    }
}

impl RhaiCalendar {
    pub fn load(name: &str, path: &Path) -> Result<Self, String> {
        let script = fs::read_to_string(path)
            .map_err(|error| format!("读取历法 {} 失败: {error}", path.display()))?;
        Self::load_script(name, &script)
    }

    pub fn load_script(name: &str, script: &str) -> Result<Self, String> {
        let engine = limited_engine();
        let ast = engine
            .compile(script)
            .map_err(|error| format!("历法 {name} 编译失败: {error}"))?;
        Ok(Self {
            engine,
            ast,
            name: name.into(),
        })
    }

    fn call(&self, entry: &str, args: impl rhai::FuncArgs) -> Result<Dynamic, String> {
        self.engine
            .call_fn(&mut Scope::new(), &self.ast, entry, args)
            .map_err(|error| format!("历法 {} 的 {entry} 执行失败: {error}", self.name))
    }

    fn ints(&self, values: &[i64]) -> rhai::Array {
        values.iter().copied().map(Dynamic::from_int).collect()
    }

    pub fn metadata(&self) -> Result<Value, String> {
        let output = self.call("metadata", ())?;
        from_dynamic(&output).map_err(|error| format!("历法 metadata 无效: {error}"))
    }

    pub fn normalize(&self, values: &[i64]) -> Result<Vec<i64>, String> {
        let output = self
            .call("normalize", (self.ints(values),))?
            .try_cast::<rhai::Array>()
            .ok_or("normalize 必须返回数组")?;
        output
            .into_iter()
            .map(|item| item.as_int().map_err(|_| "normalize 必须返回整数数组".into()))
            .collect()
    }

    pub fn to_tick(&self, values: &[i64]) -> Result<i64, String> {
        self.call("to_tick", (self.ints(values),))?
            .as_int()
            .map_err(|_| "to_tick 必须返回整数".into())
    }

    pub fn humanize(&self, values: &[i64]) -> Result<String, String> {
        self.call("humanize", (self.ints(values),))?
            .into_string()
            .map_err(|_| "humanize 必须返回字符串".into())
    }
}

#[derive(Debug)]
pub struct Calendar {
    inner: RhaiCalendar,
    component_count: usize,
}

impl Calendar {
    // name 是 story.date_mode：内置 gregorian/gregorian_en，或在顶层 calendar 注册的自定义历法
    pub fn load(name: &str, spec: Option<&CalendarSpec>, base: &Path) -> Result<Self, String> {
        let inner = match name {
            "gregorian" => {
                RhaiCalendar::load_script(name, &format!("{GREGORIAN_CORE}\n{GREGORIAN_CN}"))?
            }
            "gregorian_en" => {
                RhaiCalendar::load_script(name, &format!("{GREGORIAN_CORE}\n{GREGORIAN_EN}"))?
            }
            other => {
                let spec = spec
                    .ok_or_else(|| format!("未注册的历法 `{other}`，请在顶层 calendar 中注册"))?;
                spec.validate(other)?;
                RhaiCalendar::load(other, &base.join(&spec.source))?
            }
        };
        let metadata = inner.metadata()?;
        Ok(Self {
            component_count: metadata
                .get("components")
                .and_then(Value::as_u64)
                .unwrap_or(6) as usize,
            inner,
        })
    }

    pub fn metadata(&self, name: &str) -> CalendarMetadata {
        let value = self.inner.metadata().unwrap_or(Value::Null);
        CalendarMetadata {
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
            components: self.component_count,
        }
    }

    // 脚本只认 component_count 个分量：超长是写错了，报错；缺尾补零
    fn pad(&self, values: &[i64]) -> Result<Vec<i64>, String> {
        if values.len() > self.component_count {
            return Err(format!(
                "时间分量过多：{} 个，该历法最多 {} 个",
                values.len(),
                self.component_count
            ));
        }
        let mut v = values.to_vec();
        v.resize(self.component_count, 0);
        Ok(v)
    }

    pub fn normalize(&self, values: &[i64]) -> Result<Vec<i64>, String> {
        let padded = self.pad(values)?;
        let result = self.inner.normalize(&padded)?;
        if result.len() != self.component_count {
            return Err(format!(
                "normalize 必须返回 {} 个时间分量",
                self.component_count
            ));
        }
        Ok(result)
    }

    pub fn humanize(&self, values: &[i64]) -> Result<String, String> {
        self.inner.humanize(&self.pad(values)?)
    }

    pub fn to_tick(&self, values: &[i64]) -> Result<i64, String> {
        self.inner.to_tick(&self.pad(values)?)
    }
}

// 脚本来自用户文件，限制资源防止失控
fn limited_engine() -> Engine {
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
