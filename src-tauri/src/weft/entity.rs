//! 实体领域模型；输入解析、图构建、校验和标签计算由内部模块负责。

mod collection;
mod input;
mod tags;
mod validation;

pub use collection::EntityCollection;
pub(crate) use input::{RawEntity, RawEntityLink};

use std::sync::Arc;

use serde::{Serialize, Serializer};

use crate::calendar::Calendar;
use crate::weft::errors::DocumentError;
use crate::weft::phase::Phase;

// 集合与归属边共享同一份实体；归属图经过无环校验，不会形成 Arc 循环。
#[derive(Debug, Serialize)]
pub struct Entity {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    // 原始 Phase 用于偏移运算，展示归一化不会改写它。
    pub base_time: Option<Phase>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_time_humanizer: Option<String>,
    pub description: Option<String>,
    #[serde(serialize_with = "serialize_parents")]
    pub belong_to: Vec<Arc<Entity>>,
}

fn serialize_parents<S: Serializer>(
    parents: &[Arc<Entity>],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    parents
        .iter()
        .map(|parent| &parent.id)
        .collect::<Vec<_>>()
        .serialize(serializer)
}

impl Entity {
    fn new(
        id: String,
        raw: RawEntity,
        belong_to: Vec<Arc<Entity>>,
        cal: &Calendar,
    ) -> Result<Self, DocumentError> {
        let base_time_humanizer = raw
            .base_time
            .as_ref()
            .map(|phase| {
                let normalized = cal.normalize(&phase.flatten(cal.component_count())?)?;
                cal.humanize(&normalized)
            })
            .transpose()?;
        Ok(Self {
            id,
            avatar: raw.avatar,
            base_time: raw.base_time,
            base_time_humanizer,
            description: raw.description,
            belong_to,
        })
    }
}

#[derive(Debug, Serialize)]
pub struct EntityLink {
    #[serde(serialize_with = "serialize_endpoints")]
    pub entities: (Arc<Entity>, Arc<Entity>),
    pub relations: String,
}

fn serialize_endpoints<S: Serializer>(
    entities: &(Arc<Entity>, Arc<Entity>),
    serializer: S,
) -> Result<S::Ok, S::Error> {
    (&entities.0.id, &entities.1.id).serialize(serializer)
}
