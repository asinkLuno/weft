use std::{collections::HashMap, path::Path, sync::Arc};

use rune::{Unit, Vm, runtime::RuntimeContext};
use serde_json::{Value, json};

use crate::calendar::Calendar;
use crate::plugin_loader;
use crate::weft::errors::{DocumentError, SourceSpan, ValidationError};

// 可计算属性运行时：脚本接收实体上下文，calculable_prop 的返回值直接作为
// 实体 extra_props 中该属性注册 ID 下的值，不生成实体，也不改归属。
#[derive(Debug)]
pub struct CalculablePropRuntime {
    runtime: Arc<RuntimeContext>,
    // 注册 ID → 编译好的脚本，加载时即完成读取、编译与接口检查。
    scripts: HashMap<String, Arc<Unit>>,
    // 文档级上下文常量，加载时确定，实体段只提供 entity。
    calendar: Value,
    lang: String,
}

impl CalculablePropRuntime {
    // 注册表来自 story.calculable_props，脚本路径相对故事文件；
    // calendar 元信息与 lang 随运行时一起固定，切换语言时整个文档会重新解析。
    pub fn load(
        specs: &HashMap<String, (String, SourceSpan)>,
        base: &Path,
        calendar: &Calendar,
        lang: &str,
    ) -> Result<Self, DocumentError> {
        let runtime = plugin_loader::host_runtime();
        let mut scripts = HashMap::new();
        let mut names: Vec<_> = specs.keys().collect();
        names.sort_unstable();
        for name in names {
            let (source, span) = &specs[name];
            let unit = load_script(name, source, base, &runtime)
                .map_err(|err| DocumentError::validation_at(err, span.clone()))?;
            scripts.insert(name.clone(), unit);
        }
        Ok(Self {
            runtime,
            scripts,
            calendar: json!({
                "id": calendar.id(),
                "components": calendar.component_count(),
                "minimums": calendar.minimums(),
            }),
            lang: lang.to_owned(),
        })
    }

    // 计算一个属性：脚本上下文由文档级常量与实体段提供的 entity 拼成；
    // 顶层 () 表示不写入，其余返回值经 serde 转成 JSON 交给实体。
    pub fn calculable_prop(&self, name: &str, entity: Value) -> Result<Option<Value>, DocumentError> {
        let unit = self
            .scripts
            .get(name)
            .ok_or_else(|| ValidationError::UnknownCalculableProp {
                prop: name.to_owned(),
            })?;
        let context = json!({
            "api": plugin_loader::API_VERSION,
            "language": self.lang,
            "calendar": self.calendar,
            "entity": entity,
        });
        // JSON 与脚本值都过 serde：null ↔ ()，数组/对象直接对应。
        let argument = plugin_loader::to_rune_value(&context).map_err(|details| {
            ValidationError::CalculablePropArgument {
                prop: name.to_owned(),
                details,
            }
        })?;
        let mut vm = Vm::new(Arc::clone(&self.runtime), Arc::clone(unit));
        let output =
            plugin_loader::call(&mut vm, "calculable_prop", (argument,)).map_err(|error| {
                ValidationError::CalculablePropCall {
                    prop: name.to_owned(),
                    entry: "calculable_prop".to_owned(),
                    details: error.to_string(),
                }
            })?;
        if output.into_unit().is_ok() {
            return Ok(None);
        }
        match serde_json::to_value(&output) {
            Ok(value) => Ok(Some(value)),
            Err(_) => Err(ValidationError::CalculablePropInvalidReturn {
                prop: name.to_owned(),
                entry: "calculable_prop".to_owned(),
                expected: "a string, int, float, bool, array or object of them".to_owned(),
            }
            .into()),
        }
    }
}

fn load_script(
    name: &str,
    source: &str,
    base: &Path,
    runtime: &Arc<RuntimeContext>,
) -> Result<Arc<Unit>, ValidationError> {
    let path = base.join(source);
    if !plugin_loader::is_rune_script(&path) {
        return Err(ValidationError::InvalidCalculablePropPath {
            prop: name.to_owned(),
            path: source.to_owned(),
        });
    }
    let script = plugin_loader::read_script(&path).map_err(|details| {
        ValidationError::CalculablePropFileRead {
            prop: name.to_owned(),
            details,
        }
    })?;
    let display = path.display().to_string();
    let unit = plugin_loader::compile_cached(&script, &display).map_err(|details| {
        ValidationError::CalculablePropCompile {
            prop: name.to_owned(),
            details,
        }
    })?;
    let missing = plugin_loader::missing_functions(&unit, runtime, &["calculable_prop"]);
    if !missing.is_empty() {
        return Err(ValidationError::CalculablePropMissingInterface {
            prop: name.to_owned(),
            functions: missing.join(", "),
        });
    }
    Ok(unit)
}
