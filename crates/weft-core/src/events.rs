//! 事件列表名称映射到事件 ID 列表，保留文档中的顺序。

use std::collections::BTreeMap;

use crate::errors::{ByteSpan, Diagnostic, ValidationError};
use crate::event::RawEvent;
use crate::lint::suggest_similar;

pub type Events = Vec<String>;

pub(crate) fn check_collecting(
    event_lists: &BTreeMap<String, (ByteSpan, Vec<(String, ByteSpan)>)>,
    event_defs: &BTreeMap<String, RawEvent>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let event_names: Vec<&str> = event_defs.keys().map(String::as_str).collect();
    for (name, (_, refs)) in event_lists {
        for (id, span) in refs {
            if !event_defs.contains_key(id) {
                let mut diag = Diagnostic::from_validation(
                    ValidationError::UnknownEventsEvent {
                        events: name.clone(),
                        event: id.clone(),
                    },
                    Some(*span),
                );
                if let Some(suggestion) = suggest_similar(id, &event_names) {
                    diag = diag.with_suggestion(suggestion);
                }
                diagnostics.push(diag);
            }
        }
    }
}
