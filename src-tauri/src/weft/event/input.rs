use serde::Deserialize;
use serde_yaml_ng::Value;

// YAML 输入；时间保留原始值，构建 EventItem 时经 Phase::from_yaml_scalar 解析。
#[derive(Debug, Deserialize)]
pub(crate) struct RawEvent {
    pub start_time: Value,
    pub end_time: Option<Value>,
    #[serde(default)]
    pub entities: Vec<String>,
    pub description: Option<String>,
}
