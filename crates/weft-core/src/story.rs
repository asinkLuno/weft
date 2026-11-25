use std::{collections::HashMap, path::Path};

use serde::Serialize;

use crate::calculable_prop::CalculablePropRuntime;
use crate::calendar::{Calendar, CalendarMeta};
use crate::errors::{ByteSpan, Diagnostic, ValidationError};
use crate::plugin_loader;

// 输入阶段只包含元数据和脚本路径（附带源码位置以便诊断定位）。
#[derive(Debug)]
pub(crate) struct RawStory {
    pub title: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub date_mode: Option<(String, ByteSpan)>,
    pub calculable_props: HashMap<String, (String, ByteSpan)>,
    pub block_span: ByteSpan,
}

/// 故事构建所需的路径和语言环境。
pub(crate) struct StoryContext<'a> {
    pub base: &'a Path,
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
    /// 构建故事运行时，收集全部历法/可计算属性加载错误。
    pub(crate) fn build_collecting(
        raw: RawStory,
        context: StoryContext<'_>,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Self> {
        let StoryContext { base, lang } = context;

        // 留空使用内嵌公历，自定义脚本路径相对于故事文件。
        let calendar = match raw
            .date_mode
            .as_ref()
            .filter(|(source, _)| !source.trim().is_empty())
        {
            Some((source, span)) => {
                let path = base.join(source);
                if !plugin_loader::is_rune_script(&path) {
                    diagnostics.push(Diagnostic::from_validation(
                        ValidationError::InvalidCalendarPath {
                            path: path.display().to_string(),
                        },
                        Some(*span),
                    ));
                    return None;
                }
                match Calendar::new(&path, lang) {
                    Ok(cal) => cal,
                    Err(err) => {
                        diagnostics.push(err.into_diagnostic(Some(*span)));
                        return None;
                    }
                }
            }
            None => match Calendar::builtin_gregorian(lang) {
                Ok(cal) => cal,
                Err(err) => {
                    diagnostics.push(err.into_diagnostic(Some(raw.block_span)));
                    return None;
                }
            },
        };

        let date = calendar.meta().clone();
        let calculable_prop_runtime = CalculablePropRuntime::load_collecting(
            &raw.calculable_props,
            base,
            &calendar,
            lang,
            diagnostics,
        );

        Some(Self {
            title: raw.title,
            description: raw.description,
            author: raw.author,
            date,
            calendar,
            calculable_prop_runtime,
        })
    }
}
