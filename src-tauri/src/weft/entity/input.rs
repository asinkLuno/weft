//! YAML 输入结构与时间解析。

use serde::{Deserialize, Deserializer, de};

use crate::weft::phase::Phase;

// 可计算标签仅用于输入阶段，执行结果在构建 Entity 时成为 belong_to 引用。
#[derive(Debug, Default, Deserialize)]
pub(crate) struct RawEntity {
    #[serde(default, deserialize_with = "deserialize_base_time")]
    pub base_time: Option<Phase>,
    pub description: Option<String>,
    pub avatar: Option<String>,
    #[serde(default)]
    pub belong_to: Vec<String>,
    #[serde(default)]
    pub calculable_tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawEntityLink {
    pub entities: (String, String),
    pub relations: String,
}

// 单个整数仍等价于单分量 Phase；列表支持递归嵌套。
fn deserialize_base_time<'de, D>(d: D) -> Result<Option<Phase>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_yaml_ng::Value::deserialize(d)?;
    let value = match value {
        serde_yaml_ng::Value::Null => return Ok(None),
        serde_yaml_ng::Value::Number(_) => serde_yaml_ng::Value::Sequence(vec![value]),
        value => value,
    };
    Phase::from_yaml(&value)
        .map(Some)
        .map_err(de::Error::custom)
}
