//! 实体模型：编译期输入结构、构建上下文与运行时结构，
//! 以及头像静态校验和可计算属性求值流程。

use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::Path,
};

use serde::Serialize;
use serde_json::{Value as JsonValue, json};

use crate::calculable_prop::CalculablePropRuntime;
use crate::calendar::{Calendar, Moment};
use crate::errors::{ByteSpan, Diagnostic, ValidationError};
use crate::phase::Phase;

// ---- 输入结构 ----

#[derive(Debug)]
pub(crate) struct RawEntity {
    pub name_span: ByteSpan,
    pub base_time: Option<(Phase, ByteSpan)>,
    pub description: Option<String>,
    pub avatar: Option<(String, ByteSpan)>,
    pub calculable_props: Vec<(String, ByteSpan)>,
    pub extra_props: HashMap<String, JsonValue>,
}

// ---- 运行时结构 ----

/// 已完成头像解析和基准时间展示归一化的实体；事件与关系边通过节点 ID 引用。
#[derive(Debug, Serialize)]
pub struct Entity {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    pub base_time: Option<Phase>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_time_humanizer: Option<String>,
    pub description: Option<String>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub extra_props: HashMap<String, JsonValue>,
}

// ---- 静态校验（不依赖 Rune 脚本） ----

/// 校验实体头像文件路径，并收集全部错误。
pub(crate) fn validate_entities_static(
    entities: &BTreeMap<String, RawEntity>,
    base: &Path,
    check_files: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if check_files {
        validate_avatars(entities, base, diagnostics);
    }
}

fn validate_avatars(
    entities: &BTreeMap<String, RawEntity>,
    base: &Path,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (id, input) in entities {
        let Some((source, span)) = input
            .avatar
            .as_ref()
            .filter(|(source, _)| !source.trim().is_empty())
        else {
            continue;
        };
        let valid = fs::canonicalize(base.join(source))
            .map(|path| path.is_file() && is_image(&path))
            .unwrap_or(false);
        if !valid {
            diagnostics.push(Diagnostic::from_validation(
                ValidationError::InvalidAvatarPath {
                    entity: id.clone(),
                    path: source.clone(),
                },
                Some(*span),
            ));
        }
    }
}

// ---- 运行时构建（依赖历法与可计算属性脚本） ----

pub(crate) struct EntityBuildContext<'a> {
    pub base: &'a Path,
    pub calendar: &'a Calendar,
    pub calculable_props: &'a CalculablePropRuntime,
}

/// 将静态校验通过的实体表结合历法与可计算属性构建成运行时模型，继续收集运行时错误。
/// 同时返回各实体解析后的基准时间 `Moment`，供事件构建阶段计算相对时间差。
pub(crate) fn build_entities_runtime(
    raw_entities: BTreeMap<String, RawEntity>,
    context: EntityBuildContext<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) -> (HashMap<String, Entity>, HashMap<String, Moment>) {
    let mut entities = HashMap::with_capacity(raw_entities.len());
    let mut base_time_moments = HashMap::new();

    for (id, input) in raw_entities {
        let base_time_moment = input
            .base_time
            .as_ref()
            .and_then(|(phase, span)| match context.calendar.resolve(phase) {
                Ok(moment) => Some(moment),
                Err(err) => {
                    diagnostics.push(err.into_diagnostic(Some(*span)));
                    None
                }
            });
        let base_time_humanizer = base_time_moment.as_ref().and_then(|moment| {
            match context.calendar.humanize(moment) {
                Ok(text) => Some(text),
                Err(err) => {
                    diagnostics.push(err.into_diagnostic(input.base_time.as_ref().map(|(_, s)| *s)));
                    None
                }
            }
        });
        let avatar = input
            .avatar
            .map(|(source, _)| source)
            .filter(|source| !source.trim().is_empty())
            .and_then(|source| fs::canonicalize(context.base.join(source)).ok())
            .filter(|path| path.is_file())
            .map(|path| path.to_string_lossy().into_owned());
        let mut entity = Entity {
            id: id.clone(),
            avatar,
            base_time: input.base_time.map(|(phase, _)| phase),
            base_time_humanizer,
            description: input.description,
            extra_props: input.extra_props,
        };
        apply_calculable_props(
            &mut entity,
            &input.calculable_props,
            base_time_moment.as_ref(),
            &context,
            diagnostics,
        );
        if let Some(moment) = base_time_moment {
            base_time_moments.insert(id.clone(), moment);
        }
        entities.insert(id, entity);
    }

    (entities, base_time_moments)
}

fn apply_calculable_props(
    entity: &mut Entity,
    props: &[(String, ByteSpan)],
    base_time_moment: Option<&Moment>,
    context: &EntityBuildContext<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if props.is_empty() {
        return;
    }
    let base_time = base_time_moment.map(|moment| {
        json!({
            "values": moment.values,
            "precision": moment.precision,
            "tick": moment.tick,
        })
    });
    let raw_extra_props = entity.extra_props.clone();
    for (name, span) in props {
        let input = json!({
            "id": entity.id,
            "base_time": base_time,
            "description": entity.description,
            "extra_props": raw_extra_props,
        });
        match context.calculable_props.calculable_prop(name, input) {
            Ok(Some(value)) => {
                entity.extra_props.insert(name.clone(), value);
            }
            Ok(None) => {}
            Err(err) => {
                diagnostics.push(err.into_diagnostic(Some(*span)));
            }
        }
    }
}

// ---- 校验工具 ----

const IMAGE_EXTENSIONS: [&str; 8] = ["bmp", "gif", "ico", "jpeg", "jpg", "png", "svg", "webp"];

fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            IMAGE_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
        })
}
