use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::weft::errors::{DocumentError, ValidationError};
use crate::weft::plugin::CalendarSpec;

/// story 段：作品的元数据与历法选择
#[derive(Debug, Serialize, Deserialize)]
pub struct Story {
    pub title: String,
    pub description: Option<String>,
    pub author: Option<String>,
    // 历法名：内置 gregorian，或顶层 calendar 的注册名；缺省 gregorian
    pub date_mode: Option<String>,
}

impl Story {
    pub(crate) fn date_mode(&self) -> &str {
        self.date_mode.as_deref().unwrap_or("gregorian")
    }
}

// story 段检查：date_mode 只要求指向存在的历法，脚本本身的校验在 Calendar::load。
// 这是 YAML 反序列化后的第一个检查：历法不成立，后面的时间解析都没有意义
pub(crate) fn check(
    story: &Story,
    calendar: &HashMap<String, CalendarSpec>,
) -> Result<(), DocumentError> {
    let mode = story.date_mode();
    if mode != "gregorian" && !calendar.contains_key(mode) {
        return Err(ValidationError::UnregisteredCalendar {
            calendar: mode.into(),
        }
        .into());
    }
    Ok(())
}
