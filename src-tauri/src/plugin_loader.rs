//! Rune 宿主的公共上下文、编译、接口检查与限额调用。

use std::sync::Arc;

use rune::{
    Context, Diagnostics, Source, Sources, Unit, Vm,
    runtime::{GuardedArgs, RuntimeContext, Value as RuneValue, VmError, budget},
    termcolor,
};

// 脚本来自用户文件，限制指令预算防止失控；上下文不装 io（无 print/dbg），
// 原生函数不注入，预算对脚本内的全部执行生效。
pub(crate) const INSTRUCTION_BUDGET: usize = 1_000_000;

pub(crate) fn host_context() -> Context {
    Context::with_config(false).expect("构建默认 rune 上下文不会失败")
}

pub(crate) fn host_runtime() -> Arc<RuntimeContext> {
    Arc::new(
        host_context()
            .runtime()
            .expect("从默认上下文构建运行时不会失败"),
    )
}

// 编译单个脚本；失败时把诊断渲染成带源码定位的纯文本。
pub(crate) fn compile(script: &str, path: &str) -> Result<Unit, String> {
    let mut sources = Sources::new();
    sources
        .insert(
            Source::with_path(path, script, path).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
    let context = host_context();
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

// 限额执行一个入口函数。
pub(crate) fn call(
    vm: &mut Vm,
    entry: &str,
    args: impl GuardedArgs,
) -> Result<RuneValue, VmError> {
    budget::with(INSTRUCTION_BUDGET, || vm.call([entry], args)).call()
}
