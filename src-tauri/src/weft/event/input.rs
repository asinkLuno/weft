use serde::Deserialize;
use serde_yaml_ng::Value;

// YAML 输入；时间列表在构建 EventItem 时解析为 Phase。
#[derive(Debug, Deserialize)]
pub(crate) struct RawEvent {
    pub start_time: Value,
    pub end_time: Option<Value>,
    #[serde(default)]
    pub entities: Vec<String>,
    pub description: Option<String>,
}
