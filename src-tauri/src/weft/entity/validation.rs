//! 实体引用与归属图的完整性校验。

use std::collections::HashMap;

use crate::weft::errors::{DocumentError, ValidationError};

use super::{RawEntity, RawEntityLink};

pub(super) fn validate(
    entity: &HashMap<String, RawEntity>,
    links: &HashMap<String, Vec<RawEntityLink>>,
) -> Result<(), DocumentError> {
    let mut errors = Vec::new();
    for (id, input) in entity {
        for parent in &input.belong_to {
            if !entity.contains_key(parent) {
                errors.push(ValidationError::EntityUnknownParent {
                    entity: id.clone(),
                    target: parent.clone(),
                });
            }
        }
    }
    for (group, pairs) in links {
        for link in pairs {
            for id in [&link.entities.0, &link.entities.1] {
                if !entity.contains_key(id) {
                    errors.push(ValidationError::UnknownLinkEntity {
                        group: group.clone(),
                        entity: id.clone(),
                    });
                }
            }
        }
    }
    if !errors.is_empty() {
        errors.sort_by_cached_key(ToString::to_string);
        return Err(DocumentError::References { errors });
    }
    let mut state = HashMap::new();
    for id in entity.keys() {
        visit(id, entity, &mut state)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum VisitState {
    Visiting,
    Visited,
}

fn visit<'a>(
    id: &'a str,
    graph: &'a HashMap<String, RawEntity>,
    state: &mut HashMap<&'a str, VisitState>,
) -> Result<(), DocumentError> {
    match state.get(id) {
        Some(VisitState::Visiting) => {
            return Err(ValidationError::EntityCycle {
                entity: id.to_owned(),
            }
            .into());
        }
        Some(VisitState::Visited) => return Ok(()),
        _ => {}
    }
    state.insert(id, VisitState::Visiting);
    for parent in &graph[id].belong_to {
        visit(parent, graph, state)?;
    }
    state.insert(id, VisitState::Visited);
    Ok(())
}
