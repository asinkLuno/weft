//! 叙事线名称映射到事件 ID 列表，保留 YAML 中的顺序。

use crate::weft::errors::{DocumentError, ValidationError};
use crate::weft::event::RawEvent;
use std::collections::HashMap;

pub type Narrative = Vec<String>;

pub(crate) fn check(
    narratives: &HashMap<String, Narrative>,
    events: &HashMap<String, RawEvent>,
) -> Result<(), DocumentError> {
    let mut names: Vec<_> = narratives.keys().collect();
    names.sort_unstable();
    for name in names {
        for id in &narratives[name] {
            if !events.contains_key(id) {
                return Err(ValidationError::UnknownNarrativeEvent {
                    narrative: name.clone(),
                    event: id.clone(),
                }
                .into());
            }
        }
    }
    Ok(())
}
