use rhai::serde::to_dynamic;
use rhai::{AST, Dynamic, Engine, Scope};
use serde_json::{Value, json};
use std::{collections::HashMap, fs, path::Path};

use crate::plugin_loader::{limited_engine, missing_rhai_functions};
use crate::weft::errors::{DocumentError, ValidationError};

// 可计算属性运行时：脚本接收实体上下文，calculable_prop 的返回值直接作为
// 实体 extra_props 中该属性注册 ID 下的值，不生成实体，也不改归属。
#[derive(Debug)]
pub struct CalculablePropRuntime {
    engine: Engine,
    // 注册 ID → 编译好的脚本，加载时即完成读取、编译与接口检查。
    scripts: HashMap<String, AST>,
    // 脚本上下文中的文档级常量，加载时确定，实体段只提供 entity。
    date_mode: Option<String>,
    lang: String,
}

impl CalculablePropRuntime {
    // 注册表来自 story.calculatable_props，脚本路径相对故事文件；
    // date_mode 与 lang 随运行时一起固定，切换语言时整个文档会重新解析。
    pub fn load(
        specs: &HashMap<String, String>,
        base: &Path,
        date_mode: Option<&str>,
        lang: &str,
    ) -> Result<Self, DocumentError> {
        let engine = limited_engine();
        let mut scripts = HashMap::new();
        for (name, source) in specs {
            if Path::new(source)
                .extension()
                .is_none_or(|extension| extension != "rhai")
            {
                return Err(ValidationError::InvalidCalculablePropPath {
                    prop: name.clone(),
                    path: source.clone(),
                }
                .into());
            }
            let script = fs::read_to_string(base.join(source)).map_err(|error| {
                ValidationError::CalculablePropFileRead {
                    prop: name.clone(),
                    details: error.to_string(),
                }
            })?;
            let ast =
                engine
                    .compile(&script)
                    .map_err(|error| ValidationError::CalculablePropCompile {
                        prop: name.clone(),
                        details: error.to_string(),
                    })?;
            validate_interface(name, &ast)?;
            scripts.insert(name.clone(), ast);
        }
        Ok(Self {
            engine,
            scripts,
            date_mode: date_mode.map(ToOwned::to_owned),
            lang: lang.to_owned(),
        })
    }

    // 计算一个属性：脚本上下文由文档级常量与实体段提供的 entity 拼成；
    // 顶层 () 表示不写入，其余返回值转成 JSON 交给实体。
    pub fn calculable_prop(
        &self,
        name: &str,
        entity: Value,
    ) -> Result<Option<Value>, DocumentError> {
        let ast = self
            .scripts
            .get(name)
            .ok_or_else(|| ValidationError::UnknownCalculableProp {
                prop: name.to_owned(),
            })?;
        let context = json!({
            "api": 1,
            "date_mode": self.date_mode,
            "language": self.lang,
            "entity": entity,
        });
        let argument =
            to_dynamic(&context).map_err(|error| ValidationError::CalculablePropArgument {
                prop: name.to_owned(),
                details: error.to_string(),
            })?;
        let output: Dynamic = self
            .engine
            .call_fn(&mut Scope::new(), ast, "calculable_prop", (argument,))
            .map_err(|error| ValidationError::CalculablePropCall {
                prop: name.to_owned(),
                entry: "calculable_prop".to_owned(),
                details: error.to_string(),
            })?;
        if output.is_unit() {
            return Ok(None);
        }
        Ok(Some(prop_value(name, output)?))
    }
}

// 返回值支持字符串、整数、浮点、布尔与数组/map 的组合；复合结构里的 () 落为 null。
fn prop_value(prop: &str, value: Dynamic) -> Result<Value, ValidationError> {
    let invalid = || ValidationError::CalculablePropInvalidReturn {
        prop: prop.to_owned(),
        entry: "calculable_prop".to_owned(),
        expected: "a string, int, float, bool, array or map of them".to_owned(),
    };
    if value.is_unit() {
        return Ok(Value::Null);
    }
    if let Some(array) = value.clone().try_cast::<rhai::Array>() {
        return array
            .into_iter()
            .map(|item| prop_value(prop, item))
            .collect();
    }
    if let Some(map) = value.clone().try_cast::<rhai::Map>() {
        let mut object = serde_json::Map::new();
        for (key, item) in map {
            object.insert(key.to_string(), prop_value(prop, item)?);
        }
        return Ok(Value::Object(object));
    }
    if let Some(value) = value.clone().try_cast::<bool>() {
        return Ok(value.into());
    }
    if let Some(value) = value.clone().try_cast::<rhai::INT>() {
        return Ok(value.into());
    }
    if let Some(value) = value.clone().try_cast::<f64>() {
        return Ok(value.into());
    }
    value
        .try_cast::<String>()
        .map(|value| value.into())
        .ok_or_else(invalid)
}

fn validate_interface(name: &str, ast: &AST) -> Result<(), DocumentError> {
    let missing = missing_rhai_functions(ast, &[("calculable_prop", 1)]);
    if missing.is_empty() {
        Ok(())
    } else {
        Err(ValidationError::CalculablePropMissingInterface {
            prop: name.to_owned(),
            functions: missing.join(", "),
        }
        .into())
    }
}
