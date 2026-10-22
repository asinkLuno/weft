//! 叙事线名称映射到事件 ID 列表，保留 YAML 中的顺序。

use crate::weft::errors::{DocumentError, ValidationError};
use crate::weft::event::RawEvent;
use std::collections::HashMap;

pub type Narrative = Vec<String>;

pub(crate) fn check(
    narratives: &HashMap<String, Narrative>,
    events: &HashMap<String, RawEvent>,
) -> Result<(), DocumentError> {
    let mut errors = Vec::new();
    for (name, ids) in narratives {
        for id in ids {
            if !events.contains_key(id) {
                errors.push(ValidationError::UnknownNarrativeEvent {
                    narrative: name.clone(),
                    event: id.clone(),
                });
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        errors.sort_by_cached_key(ToString::to_string);
        Err(DocumentError::References { errors })
    }
}
