use rhai::serde::{from_dynamic, to_dynamic};
use rhai::{AST, Dynamic, Engine, Scope};
use serde_json::Value;
use std::{collections::HashMap, fs, path::Path};

use crate::calendar::{limited_engine, missing_rhai_functions};
use crate::model::CalendarSpec;
use crate::validation::DocumentError;

const CONSTELLATION: &str = include_str!("../rhai/constellation.rhai");

// 可计算标签运行时：脚本拿实体上下文算出归属链（字符串列表），结果并入 belong_to
#[derive(Debug)]
pub struct CalculableTagRuntime {
    engine: Engine,
    // 可计算标签名 → 编译好的脚本；计算和展示分别走 calculable_tag / humanizer
    calculable_tags: HashMap<String, AST>,
}

impl CalculableTagRuntime {
    // specs 是顶层 calculable_tag 注册表，脚本路径相对故事文件；内置星座可计算标签免注册，注册可覆盖
    pub fn load(specs: &HashMap<String, CalendarSpec>, base: &Path) -> Result<Self, DocumentError> {
        let engine = limited_engine();
        let mut calculable_tags = HashMap::new();
        for (name, spec) in specs {
            spec.validate(name, "calculable_tag")?;
            let script = fs::read_to_string(base.join(&spec.source))
                .map_err(|error| format!("读取可计算标签 {name} 失败: {error}"))?;
            let ast = engine
                .compile(&script)
                .map_err(|error| format!("可计算标签 {name} 编译失败: {error}"))?;
            validate_interface(name, &ast)?;
            calculable_tags.insert(name.clone(), ast);
        }
        for name in ["constellation"] {
            if !calculable_tags.contains_key(name) {
                let ast = engine
                    .compile(CONSTELLATION)
                    .map_err(|error| format!("内置可计算标签 {name} 编译失败: {error}"))?;
                validate_interface(name, &ast)?;
                calculable_tags.insert(name.to_string(), ast);
            }
        }
        Ok(Self {
            engine,
            calculable_tags,
        })
    }

    pub fn calculable_tag(&self, name: &str, context: &Value) -> Result<Value, String> {
        let ast = self
            .calculable_tags
            .get(name)
            .ok_or_else(|| format!("引用了未注册的可计算标签 `{name}`"))?;
        let argument = to_dynamic(context)
            .map_err(|error| format!("可计算标签 {name} 入参转换失败: {error}"))?;
        let output = self
            .engine
            .call_fn::<Dynamic>(&mut Scope::new(), ast, "calculable_tag", (argument,))
            .map_err(|error| format!("可计算标签 {name} 执行失败: {error}"))?;
        let lang = context
            .get("language")
            .and_then(Value::as_str)
            .unwrap_or("en");
        let localized = self
            .engine
            .call_fn::<Dynamic>(
                &mut Scope::new(),
                ast,
                "humanizer",
                ("chain", output, lang.to_owned()),
            )
            .map_err(|error| format!("可计算标签 {name} 展示失败: {error}"))?;
        from_dynamic(&localized).map_err(|error| format!("可计算标签 {name} 返回值无效: {error}"))
    }
}

fn validate_interface(name: &str, ast: &AST) -> Result<(), DocumentError> {
    let missing = missing_rhai_functions(ast, &[("calculable_tag", 1), ("humanizer", 3)]);
    if missing.is_empty() {
        Ok(())
    } else {
        Err(DocumentError::raw(format!(
            "可计算标签 {name} 缺少公开接口：{}",
            missing.join(", ")
        )))
    }
}
