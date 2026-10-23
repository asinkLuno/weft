use std::{collections::HashMap, path::Path};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::calculable_prop::CalculablePropRuntime;
use crate::calendar::Calendar;
use crate::weft::FromRaw;
use crate::weft::errors::{DocumentError, ValidationError};

// YAML 输入只包含元数据和脚本路径。
#[derive(Debug, Deserialize)]
pub(crate) struct RawStory {
    pub title: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub date_mode: Option<String>,
    #[serde(default)]
    pub calculatable_props: HashMap<String, String>,
}

/// 故事构建所需的路径和语言环境。
pub(crate) struct StoryContext<'a> {
    pub base: &'a Path,
    pub gregorian: &'a Path,
    pub lang: &'a str,
}

/// 已完成历法加载并持有可计算属性运行时的故事。
#[derive(Debug, Serialize)]
pub struct Story {
    pub title: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub date: Value,
    #[serde(skip)]
    pub calendar: Calendar,
    #[serde(skip)]
    pub calculable_prop_runtime: CalculablePropRuntime,
}

impl FromRaw<StoryContext<'_>> for Story {
    type Raw = RawStory;

    fn from_raw(raw: Self::Raw, context: StoryContext<'_>) -> Result<Self, DocumentError> {
        let StoryContext {
            base,
            gregorian,
            lang,
        } = context;
        // 留空使用内置公历，自定义脚本路径相对于故事文件。
        let date_mode = raw
            .date_mode
            .as_deref()
            .filter(|source| !source.trim().is_empty());
        let path = match date_mode {
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
        let calculable_prop_runtime =
            CalculablePropRuntime::load(&raw.calculatable_props, base, date_mode, lang)?;
        Ok(Self {
            title: raw.title,
            description: raw.description,
            author: raw.author,
            date,
            calendar,
            calculable_prop_runtime,
        })
    }
}
