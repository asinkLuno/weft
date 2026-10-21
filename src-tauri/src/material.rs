use rhai::serde::{from_dynamic, to_dynamic};
use rhai::{Dynamic, Engine, Scope, AST};
use serde_json::Value;
use std::{collections::HashMap, fs, path::Path};

use crate::calendar::limited_engine;
use crate::model::CalendarSpec;

const CONSTELLATION_CN: &str = include_str!("../rhai/constellation_cn.rhai");
const CONSTELLATION_EN: &str = include_str!("../rhai/constellation_en.rhai");

// 材料运行时：脚本拿实体上下文算出归属链（字符串列表），结果并入 belong_to（旧版 material）
#[derive(Debug)]
pub struct MaterialRuntime {
    engine: Engine,
    // 材料名 → 编译好的脚本，入口约定 fn material(ctx)
    materials: HashMap<String, AST>,
}

impl MaterialRuntime {
    // specs 是顶层 material 注册表，脚本路径相对故事文件；内置星座材料免注册，注册可覆盖
    pub fn load(specs: &HashMap<String, CalendarSpec>, base: &Path) -> Result<Self, String> {
        let engine = limited_engine();
        let mut materials = HashMap::new();
        for (name, spec) in specs {
            spec.validate(name, "material")?;
            let script = fs::read_to_string(base.join(&spec.source))
                .map_err(|error| format!("读取材料 {name} 失败: {error}"))?;
            let ast = engine
                .compile(&script)
                .map_err(|error| format!("材料 {name} 编译失败: {error}"))?;
            materials.insert(name.clone(), ast);
        }
        for (name, script) in [
            ("constellation", CONSTELLATION_CN),
            ("constellation_cn", CONSTELLATION_CN),
            ("constellation_en", CONSTELLATION_EN),
        ] {
            if !materials.contains_key(name) {
                let ast = engine
                    .compile(script)
                    .map_err(|error| format!("内置材料 {name} 编译失败: {error}"))?;
                materials.insert(name.to_string(), ast);
            }
        }
        Ok(Self { engine, materials })
    }

    pub fn material(&self, name: &str, context: &Value) -> Result<Value, String> {
        let ast = self
            .materials
            .get(name)
            .ok_or_else(|| format!("引用了未注册的材料 `{name}`"))?;
        let argument =
            to_dynamic(context).map_err(|error| format!("材料 {name} 入参转换失败: {error}"))?;
        let output = self
            .engine
            .call_fn::<Dynamic>(&mut Scope::new(), ast, "material", (argument,))
            .map_err(|error| format!("材料 {name} 执行失败: {error}"))?;
        from_dynamic(&output).map_err(|error| format!("材料 {name} 返回值无效: {error}"))
    }
}
