//! Rhai 宿主的公共资源限制与接口检查。

use rhai::{AST, Engine, FnAccess};

pub(crate) fn missing_rhai_functions(ast: &AST, required: &[(&str, usize)]) -> Vec<String> {
    required
        .iter()
        .filter(|(name, arity)| {
            !ast.iter_functions().any(|function| {
                function.name == *name
                    && function.params.len() == *arity
                    && function.access == FnAccess::Public
            })
        })
        .map(|(name, arity)| format!("{name}/{arity}"))
        .collect()
}

// 脚本来自用户文件，限制资源防止失控
pub(crate) fn limited_engine() -> Engine {
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
