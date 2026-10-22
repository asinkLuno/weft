use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use crate::calculable_tag::CalculableTagRuntime;
use crate::calendar::Calendar;

use serde::{Deserialize, Serialize};

use crate::weft::errors::{DocumentError, ValidationError};
use crate::weft::plugin::CalendarSpec;

/// story 段：作品的元数据与历法选择
#[derive(Debug, Serialize, Deserialize)]
pub struct Story {
    pub title: String,
    pub description: Option<String>,
    pub author: Option<String>,
    // 自定义历法 Rhai 脚本路径，相对故事文件；缺省或留空使用内置公历。
    pub date_mode: Option<String>,
}

impl Story {
    pub(crate) fn date_mode(&self) -> Option<&str> {
        self.date_mode
            .as_deref()
            .filter(|source| !source.trim().is_empty())
    }
}

// story 阶段统一加载插件，后续各段只使用已编译的运行时。
pub(crate) struct StoryPlugins {
    pub calendar: Calendar,
    pub calculable_tags: CalculableTagRuntime,
}

impl Story {
    fn calendar_path(&self, base: &Path, gregorian: &Path) -> Result<PathBuf, DocumentError> {
        let path = match self.date_mode() {
            Some(source) => base.join(source),
            None => gregorian.to_path_buf(),
        };
        if path.extension().is_none_or(|extension| extension != "rhai") || !path.is_file() {
            return Err(ValidationError::InvalidCalendarPath {
                path: path.display().to_string(),
            }
            .into());
        }
        Ok(path)
    }

    pub(crate) fn load_plugins(
        &self,
        calculable_tags: &HashMap<String, CalendarSpec>,
        base: &Path,
        gregorian: &Path,
        lang: &str,
    ) -> Result<StoryPlugins, DocumentError> {
        let path = self.calendar_path(base, gregorian)?;
        Ok(StoryPlugins {
            calendar: Calendar::new(&path, lang)?,
            calculable_tags: CalculableTagRuntime::load(calculable_tags, base)?,
        })
    }
}
