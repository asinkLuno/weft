use rhai::serde::{from_dynamic, to_dynamic};
use rhai::{AST, Dynamic, Engine, Scope};
use serde_json::Value;
use std::{collections::HashMap, fs, path::Path};

use crate::plugin_loader::{limited_engine, missing_rhai_functions};
use crate::weft::errors::{DocumentError, ValidationError};

// 可计算标签运行时：脚本拿实体上下文算出归属链（字符串或 map 条目），结果并入 belong_to
#[derive(Debug)]
pub struct CalculableTagRuntime {
    engine: Engine,
    // 可计算标签名 → 编译好的脚本；计算和展示分别走 calculable_tag / humanizer
    calculable_tags: HashMap<String, AST>,
}

impl CalculableTagRuntime {
    // 注册表来自 story.calculatable_tags，脚本路径相对故事文件。
    pub fn load(specs: &HashMap<String, String>, base: &Path) -> Result<Self, DocumentError> {
        let engine = limited_engine();
        let mut calculable_tags = HashMap::new();
        for (name, source) in specs {
            if Path::new(source)
                .extension()
                .is_none_or(|extension| extension != "rhai")
            {
                return Err(ValidationError::InvalidCalculableTagPath {
                    tag: name.clone(),
                    path: source.clone(),
                }
                .into());
            }
            let script = fs::read_to_string(base.join(source)).map_err(|error| {
                ValidationError::CalculableTagFileRead {
                    tag: name.clone(),
                    details: error.to_string(),
                }
            })?;
            let ast =
                engine
                    .compile(&script)
                    .map_err(|error| ValidationError::CalculableTagCompile {
                        tag: name.clone(),
                        details: error.to_string(),
                    })?;
            validate_interface(name, &ast)?;
            calculable_tags.insert(name.clone(), ast);
        }
        Ok(Self {
            engine,
            calculable_tags,
        })
    }

    // 每个注册 ID 对应脚本生成的本地化名称和描述。
    pub fn serialize(&self, lang: &str) -> Result<HashMap<String, Value>, DocumentError> {
        self.calculable_tags
            .iter()
            .map(|(id, ast)| {
                let output = self.call(id, ast, "serialize", (lang.to_owned(),))?;
                let value: Value =
                    from_dynamic(&output).map_err(|_| invalid_return(id, "serialize", "map"))?;
                if !value.is_object() {
                    return Err(invalid_return(id, "serialize", "map").into());
                }
                Ok((id.clone(), value))
            })
            .collect()
    }

    // 计算按语言无关的 id 进行，展示统一走 humanizer(kind, value, language)；
    // 条目除 id 外可携带可选的 description 与 base_time。
    pub fn calculable_tag(
        &self,
        name: &str,
        context: &Value,
        lang: &str,
    ) -> Result<Vec<ChainEntry>, DocumentError> {
        let ast = self.calculable_tags.get(name).ok_or_else(|| {
            ValidationError::UnknownCalculableTag {
                tag: name.to_owned(),
            }
        })?;
        let argument =
            to_dynamic(context).map_err(|error| ValidationError::CalculableTagArgument {
                tag: name.to_owned(),
                details: error.to_string(),
            })?;
        let output = self.call(name, ast, "calculable_tag", (argument,))?;
        let entries: Vec<_> = output
            .try_cast::<rhai::Array>()
            .ok_or_else(|| invalid_return(name, "calculable_tag", "string or map entries"))?
            .into_iter()
            .map(|item| parse_entry(name, item))
            .collect::<Result<_, _>>()?;
        let ids: rhai::Array = entries
            .iter()
            .map(|entry| Dynamic::from(entry.name.clone()))
            .collect();
        let localized = self.call(name, ast, "humanizer", ("chain", ids, lang.to_owned()))?;
        let names: Vec<String> = from_dynamic(&localized)
            .map_err(|_| invalid_return(name, "humanizer", "string[]"))?;
        if names.len() != entries.len() {
            return Err(invalid_return(
                name,
                "humanizer",
                "string[] matching the chain length",
            )
            .into());
        }
        Ok(entries
            .into_iter()
            .zip(names)
            .map(|(entry, name)| ChainEntry { name, ..entry })
            .collect())
    }

    fn call(
        &self,
        name: &str,
        ast: &AST,
        entry: &str,
        args: impl rhai::FuncArgs,
    ) -> Result<Dynamic, ValidationError> {
        self.engine
            .call_fn(&mut Scope::new(), ast, entry, args)
            .map_err(|error| ValidationError::CalculableTagCall {
                tag: name.to_owned(),
                entry: entry.to_owned(),
                details: error.to_string(),
            })
    }
}

/// 归属链条目：name 是 humanizer 的结果并直接用作实体 ID；
/// description 与 base_time 可选，base_time 是历法时间分量。
#[derive(Debug)]
pub struct ChainEntry {
    pub name: String,
    pub description: Option<String>,
    pub base_time: Option<Vec<i64>>,
}

// 条目是纯 id 字符串，或字段受限的 map：id 必填，description 与 base_time
// 可选（缺省或 () 均视为未提供），一旦出现就校验类型，不允许未知键。
fn parse_entry(tag: &str, item: Dynamic) -> Result<ChainEntry, ValidationError> {
    if let Some(name) = item.clone().try_cast::<String>() {
        return Ok(ChainEntry {
            name,
            description: None,
            base_time: None,
        });
    }
    let map = item
        .try_cast::<rhai::Map>()
        .ok_or_else(|| invalid_return(tag, "calculable_tag", "string or map entry"))?;
    if let Some(key) = map
        .keys()
        .find(|key| !["id", "description", "base_time"].contains(&key.as_str()))
    {
        return Err(invalid_return(
            tag,
            "calculable_tag",
            &format!("map entry keys id/description/base_time only, got {key}"),
        ));
    }
    let name = map
        .get("id")
        .cloned()
        .and_then(|value| value.try_cast::<String>())
        .ok_or_else(|| invalid_return(tag, "calculable_tag", "map entry with id string"))?;
    let description = optional_string(tag, "description", map.get("description"))?;
    let base_time = optional_components(tag, map.get("base_time"))?;
    Ok(ChainEntry {
        name,
        description,
        base_time,
    })
}

fn optional_string(
    tag: &str,
    field: &str,
    value: Option<&Dynamic>,
) -> Result<Option<String>, ValidationError> {
    match value {
        None => Ok(None),
        Some(value) if value.is_unit() => Ok(None),
        Some(value) => {
            value.clone().into_string().map(Some).map_err(|_| {
                invalid_return(tag, "calculable_tag", &format!("map entry with {field} string"))
            })
        }
    }
}

// base_time 与实体输入的时间分量同构：非空整数列表，长度由历法在展平时校验。
fn optional_components(
    tag: &str,
    value: Option<&Dynamic>,
) -> Result<Option<Vec<i64>>, ValidationError> {
    let invalid = || invalid_return(tag, "calculable_tag", "map entry with base_time int[]");
    match value {
        None => Ok(None),
        Some(value) if value.is_unit() => Ok(None),
        Some(value) => {
            let components = value
                .clone()
                .try_cast::<rhai::Array>()
                .ok_or_else(invalid)?
                .into_iter()
                .map(|item| item.as_int().map_err(|_| invalid()))
                .collect::<Result<Vec<_>, _>>()?;
            if components.is_empty() {
                return Err(invalid());
            }
            Ok(Some(components))
        }
    }
}

fn validate_interface(name: &str, ast: &AST) -> Result<(), DocumentError> {
    let missing = missing_rhai_functions(
        ast,
        &[("calculable_tag", 1), ("humanizer", 3), ("serialize", 1)],
    );
    if missing.is_empty() {
        Ok(())
    } else {
        Err(ValidationError::CalculableTagMissingInterface {
            tag: name.to_owned(),
            functions: missing.join(", "),
        }
        .into())
    }
}

fn invalid_return(name: &str, entry: &str, expected: &str) -> ValidationError {
    ValidationError::CalculableTagInvalidReturn {
        tag: name.to_owned(),
        entry: entry.to_owned(),
        expected: expected.to_owned(),
    }
}
