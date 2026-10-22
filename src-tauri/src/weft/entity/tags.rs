//! 将可计算标签的执行结果展开为实体归属链。

use std::collections::HashMap;

use crate::calculable_tag::CalculableTagRuntime;
use crate::calendar::Calendar;
use crate::weft::errors::DocumentError;

use super::RawEntity;

pub(super) fn apply_calculable_tags(
    raw: &mut HashMap<String, RawEntity>,
    runtime: &CalculableTagRuntime,
    mode: Option<&str>,
    lang: &str,
    cal: &Calendar,
) -> Result<(), DocumentError> {
    let mut ids: Vec<_> = raw.keys().cloned().collect();
    ids.sort();
    for id in ids {
        for name in std::mem::take(&mut raw.get_mut(&id).expect("输入实体存在").calculable_tags)
        {
            let entity = &raw[&id];
            let base_time = entity
                .base_time
                .as_ref()
                .map(|phase| cal.normalize(&phase.flatten(cal.component_count())?))
                .transpose()?;
            let context = serde_json::json!({
                "api": 1, "date_mode": mode, "language": lang,
                "entity": { "id": id, "base_time": base_time, "description": entity.description, "belong_to": entity.belong_to },
            });
            let chain = runtime.calculable_tag(&name, &context, lang)?;
            apply_chain(raw, &id, &chain);
        }
    }
    Ok(())
}

fn apply_chain(raw: &mut HashMap<String, RawEntity>, id: &str, chain: &[String]) {
    let Some(first) = chain.first() else {
        return;
    };
    let existing: Vec<_> = chain.iter().map(|id| raw.contains_key(id)).collect();
    for (i, target) in chain.iter().enumerate() {
        if existing[i] {
            break;
        }
        raw.insert(
            target.clone(),
            RawEntity {
                belong_to: chain.get(i + 1).cloned().into_iter().collect(),
                ..RawEntity::default()
            },
        );
    }
    let owner = raw.get_mut(id).expect("可计算标签的源实体存在");
    if !owner.belong_to.contains(first) {
        owner.belong_to.push(first.clone());
    }
}
