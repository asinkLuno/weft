use crate::weft::errors::ValidationError;
use serde::{Deserialize, Serialize};

// 顶层 calendar / calculable_tag 共用的插件清单：rhai 脚本 + 固定入口约定
#[derive(Debug, Serialize, Deserialize)]
pub struct CalendarSpec {
    pub runtime: String,
    pub kind: String,
    pub api: i64,
    // 脚本路径，相对故事文件所在目录
    pub source: String,
}

impl CalendarSpec {
    pub(crate) fn validate(&self, name: &str, kind: &str) -> Result<(), ValidationError> {
        if self.runtime != "rhai" || self.kind != kind || self.api != 1 {
            return Err(ValidationError::InvalidPluginSpec {
                name: name.to_string(),
                kind: kind.to_string(),
            });
        }
        Ok(())
    }
}
