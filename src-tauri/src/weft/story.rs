use std::{collections::HashMap, path::Path};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::calculable_tag::CalculableTagRuntime;
use crate::calendar::Calendar;
use crate::weft::errors::{DocumentError, ValidationError};

// YAML 输入只包含元数据和脚本路径。
#[derive(Debug, Deserialize)]
pub(crate) struct RawStory {
    pub title: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub date_mode: Option<String>,
    #[serde(default)]
    pub calculatable_tags: HashMap<String, String>,
}

/// 已完成历法和可计算标签加载的故事。
#[derive(Debug, Serialize)]
pub struct Story {
    pub title: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub date: Value,
    pub calculatable_tags: HashMap<String, Value>,
    #[serde(skip)]
    pub calendar: Calendar,
    #[serde(skip)]
    pub calculable_tag_runtime: CalculableTagRuntime,
}

impl Story {
    pub(crate) fn new(
        raw: RawStory,
        base: &Path,
        gregorian: &Path,
        lang: &str,
    ) -> Result<Self, DocumentError> {
        // 留空使用内置公历，自定义脚本路径相对于故事文件。
        let path = match raw
            .date_mode
            .as_deref()
            .filter(|source| !source.trim().is_empty())
        {
            Some(source) => base.join(source),
            None => gregorian.to_path_buf(),
        };
        if path.extension().is_none_or(|extension| extension != "rhai") || !path.is_file() {
            return Err(ValidationError::InvalidCalendarPath {
                path: path.display().to_string(),
            }
            .into());
        }
        let calendar = Calendar::new(&path, lang)?;
        let date = calendar.serialize(lang)?;
        let calculable_tag_runtime = CalculableTagRuntime::load(&raw.calculatable_tags, base)?;
        let calculatable_tags = calculable_tag_runtime.serialize(lang)?;
        Ok(Self {
            title: raw.title,
            description: raw.description,
            author: raw.author,
            date,
            calculatable_tags,
            calendar,
            calculable_tag_runtime,
        })
    }
}
