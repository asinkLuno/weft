//! 叙事线名称映射到事件 ID 列表，保留文档中的顺序。

use std::collections::BTreeMap;

use crate::weft::errors::{DocumentError, SourceSpan, ValidationError};
use crate::weft::event::RawEvent;

pub type Narrative = Vec<String>;

pub(crate) fn check(
    narratives: &BTreeMap<String, Vec<(String, SourceSpan)>>,
    events: &BTreeMap<String, RawEvent>,
) -> Result<(), DocumentError> {
    for (name, refs) in narratives {
        for (id, span) in refs {
            if !events.contains_key(id) {
                return Err(DocumentError::validation_at(
                    ValidationError::UnknownNarrativeEvent {
                        narrative: name.clone(),
                        event: id.clone(),
                    },
                    span.clone(),
                ));
            }
        }
    }
    Ok(())
}
