use rhai::serde::{from_dynamic, to_dynamic};
use rhai::{AST, Dynamic, Engine, Scope};
use serde_json::Value;
use std::{collections::HashMap, fs, path::Path};

use crate::plugin_loader::{limited_engine, missing_rhai_functions};
use crate::weft::errors::{DocumentError, ValidationError};

// 可计算标签运行时：脚本拿实体上下文算出归属链（字符串列表），结果并入 belong_to
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

    // 计算按语言无关的 id 进行，展示统一走 humanizer(kind, value, language)
    pub fn calculable_tag(
        &self,
        name: &str,
        context: &Value,
        lang: &str,
    ) -> Result<Vec<String>, DocumentError> {
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
        from_dynamic::<Vec<String>>(&output)
            .map_err(|_| invalid_return(name, "calculable_tag", "string[]"))?;
        let localized = self.call(name, ast, "humanizer", ("chain", output, lang.to_owned()))?;
        from_dynamic(&localized).map_err(|_| invalid_return(name, "humanizer", "string[]").into())
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
