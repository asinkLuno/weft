use std::{collections::HashMap, path::Path};

use serde::Serialize;

use crate::calculable_prop::CalculablePropRuntime;
use crate::calendar::{Calendar, CalendarMeta};
use crate::plugin_loader;
use crate::weft::errors::{DocumentError, SourceSpan, ValidationError};

// 输入阶段只包含元数据和脚本路径（附带源码位置以便编辑器标红）。
#[derive(Debug)]
pub(crate) struct RawStory {
    pub title: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub date_mode: Option<(String, SourceSpan)>,
    pub calculable_props: HashMap<String, (String, SourceSpan)>,
    pub block_span: SourceSpan,
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
    pub date: CalendarMeta,
    #[serde(skip)]
    pub calendar: Calendar,
    #[serde(skip)]
    pub calculable_prop_runtime: CalculablePropRuntime,
}

impl Story {
    pub(crate) fn build(raw: RawStory, context: StoryContext<'_>) -> Result<Self, DocumentError> {
        let StoryContext {
            base,
            gregorian,
            lang,
        } = context;
        // 留空使用内置公历，自定义脚本路径相对于故事文件。
        let (path, cal_span) = match raw
            .date_mode
            .as_ref()
            .filter(|(source, _)| !source.trim().is_empty())
        {
            Some((source, span)) => (base.join(source), span.clone()),
            None => (gregorian.to_path_buf(), raw.block_span.clone()),
        };
        if !plugin_loader::is_rune_script(&path) {
            return Err(DocumentError::validation_at(
                ValidationError::InvalidCalendarPath {
                    path: path.display().to_string(),
                },
                cal_span,
            ));
        }
        let calendar = Calendar::new(&path, lang).map_err(|err| err.with_span(cal_span))?;
        let date = calendar.meta().clone();
        let calculable_prop_runtime =
            CalculablePropRuntime::load(&raw.calculable_props, base, &calendar, lang)?;
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
