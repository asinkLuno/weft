//! WEFT 语言服务内核：为桌面编辑器（进程内）与外部编辑器（`weft lsp` stdio）提供统一能力。

pub mod index;
pub mod position;
pub mod scanner;
pub mod schema;
pub mod server;
pub mod service;
pub mod stdio;
