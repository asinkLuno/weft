//! 叙事线名称映射到事件 ID 列表，保留文档中的顺序。

use std::collections::BTreeMap;

use crate::errors::{ByteSpan, Diagnostic, ValidationError};
use crate::event::RawEvent;
use crate::lint::suggest_similar;

pub type Narrative = Vec<String>;

pub(crate) fn check_collecting(
    narratives: &BTreeMap<String, (ByteSpan, Vec<(String, ByteSpan)>)>,
    events: &BTreeMap<String, RawEvent>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let event_names: Vec<&str> = events.keys().map(String::as_str).collect();
    for (name, (_, refs)) in narratives {
        for (id, span) in refs {
            if !events.contains_key(id) {
                let mut diag = Diagnostic::from_validation(
                    ValidationError::UnknownNarrativeEvent {
                        narrative: name.clone(),
                        event: id.clone(),
                    },
                    Some(*span),
                );
                if let Some(suggestion) = suggest_similar(id, &event_names) {
                    diag = diag.with_help(format!("did you mean `{suggestion}`?"));
                }
                diagnostics.push(diag);
            }
        }
    }
}
