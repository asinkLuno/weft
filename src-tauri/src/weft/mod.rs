//! Weft 领域模型、引用校验及时间结构。

pub mod entity;
pub mod errors;
pub mod event;
pub mod narrative;
pub mod phase;
pub mod story;

/// 从原始输入构建领域对象，完成该阶段的校验和依赖加载。
/// 构建所需的外部依赖通过上下文传入。
pub(crate) trait FromRaw<C = ()>: Sized {
    type Raw;

    fn from_raw(raw: Self::Raw, context: C) -> Result<Self, errors::DocumentError>;
}
