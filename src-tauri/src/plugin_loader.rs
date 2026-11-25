//! Rune 宿主的公共上下文、缓存编译、接口检查与限额调用。

use std::{
    collections::HashMap,
    fs,
    path::Path,
    sync::{Arc, Mutex, OnceLock},
};

use rune::{
    Context, Diagnostics, Source, Sources, Unit, Vm,
    runtime::{GuardedArgs, RuntimeContext, Value as RuneValue, VmError, budget},
    termcolor,
};
use serde::Deserialize as _;
use serde_json::Value as JsonValue;

/// 宿主传给插件上下文对象的 API 版本号。
pub(crate) const API_VERSION: u32 = 1;

// 脚本来自用户文件，限制指令预算与文件体积防止失控；上下文不装 io（无 print/dbg），
// 原生函数不注入，预算对脚本内的全部执行生效。
pub(crate) const INSTRUCTION_BUDGET: usize = 1_000_000;
pub(crate) const MAX_SCRIPT_BYTES: u64 = 1024 * 1024;
const COMPILE_CACHE_LIMIT: usize = 128;

fn build_context() -> Context {
    Context::with_config(false).expect("构建默认 rune 上下文不会失败")
}

// 默认运行时上下文不可变，全局只构建一次并复用。
pub(crate) fn host_runtime() -> Arc<RuntimeContext> {
    static RUNTIME: OnceLock<Arc<RuntimeContext>> = OnceLock::new();
    Arc::clone(RUNTIME.get_or_init(|| {
        Arc::new(
            build_context()
                .runtime()
                .expect("从默认上下文构建运行时不会失败"),
        )
    }))
}

pub(crate) fn is_rune_script(path: &Path) -> bool {
    path.extension().is_some_and(|extension| extension == "rn") && path.is_file()
}

pub(crate) fn read_script(path: &Path) -> Result<String, String> {
    let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
    if metadata.len() > MAX_SCRIPT_BYTES {
        return Err(format!(
            "script exceeds maximum size of {MAX_SCRIPT_BYTES} bytes"
        ));
    }
    fs::read_to_string(path).map_err(|error| error.to_string())
}

type UnitCache = Mutex<HashMap<(String, String), Arc<Unit>>>;

// 编译单个脚本并按 (路径, 源码) 缓存编译产物：编辑 .weft 时未变动的脚本免于重编译。
pub(crate) fn compile_cached(script: &str, path: &str) -> Result<Arc<Unit>, String> {
    static CACHE: OnceLock<UnitCache> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (path.to_owned(), script.to_owned());
    if let Some(unit) = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(&key)
        .cloned()
    {
        return Ok(unit);
    }

    let unit = Arc::new(compile(script, path)?);
    let mut map = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if map.len() >= COMPILE_CACHE_LIMIT {
        map.clear();
    }
    map.insert(key, Arc::clone(&unit));
    Ok(unit)
}

// 编译单个脚本；失败时把诊断渲染成带源码定位的纯文本。
fn compile(script: &str, path: &str) -> Result<Unit, String> {
    let mut sources = Sources::new();
    sources
        .insert(Source::with_path(path, script, path).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    let context = build_context();
    let mut diagnostics = Diagnostics::without_warnings();
    match rune::prepare(&mut sources)
        .with_context(&context)
        .with_diagnostics(&mut diagnostics)
        .build()
    {
        Ok(unit) => Ok(unit),
        Err(error) => {
            let mut buffer = termcolor::Buffer::no_color();
            let rendered = diagnostics
                .emit(&mut buffer, &sources)
                .ok()
                .map(|()| String::from_utf8_lossy(&buffer.into_inner()).into_owned())
                .filter(|rendered| !rendered.trim().is_empty())
                .unwrap_or_else(|| error.to_string());
            Err(rendered)
        }
    }
}

// Rune 的参数个数在运行期才确定，接口检查只看函数是否定义，参数不匹配会在调用时报错。
pub(crate) fn missing_functions(
    unit: &Arc<Unit>,
    runtime: &Arc<RuntimeContext>,
    required: &[&str],
) -> Vec<String> {
    let vm = Vm::new(Arc::clone(runtime), Arc::clone(unit));
    required
        .iter()
        .filter(|name| vm.lookup_function([**name]).is_err())
        .map(|name| (*name).to_owned())
        .collect()
}

pub(crate) fn to_rune_value(value: &JsonValue) -> Result<RuneValue, String> {
    RuneValue::deserialize(value).map_err(|error| error.to_string())
}

// 限额执行一个入口函数。
pub(crate) fn call(
    vm: &mut Vm,
    entry: &str,
    args: impl GuardedArgs,
) -> Result<RuneValue, VmError> {
    budget::with(INSTRUCTION_BUDGET, || vm.call([entry], args)).call()
}
