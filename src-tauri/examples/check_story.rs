//! 校验一份 weft 故事文档：对指定 YAML 跑与 weft_doc::parse 相同的
//! Story → Entity → Event → Narrative → apply_times 全流程，
//! 成功打印摘要，失败打印文档错误。用法：cargo run --example check_story -- <story.yaml>

#![allow(dead_code)]

#[path = "../src/calculable_prop.rs"]
mod calculable_prop;
#[path = "../src/calendar.rs"]
mod calendar;
#[path = "../src/plugin_loader.rs"]
mod plugin_loader;
#[path = "../src/weft/mod.rs"]
mod weft;

use std::{collections::HashMap, fs, path::Path, process::ExitCode};

use serde::Deserialize;
use weft::FromRaw;
use weft::event::{self, RawEvent};
use weft::narrative::{self, Narrative};
use weft::story::{RawStory, Story, StoryContext};

// 与 weft_doc::RawWeftDoc 一致的完整输入结构。
#[derive(Deserialize)]
struct RawDoc {
    story: RawStory,
    #[serde(default)]
    entity: HashMap<String, weft::entity::RawEntity>,
    #[serde(default)]
    entity_link: HashMap<String, Vec<weft::entity::EntityLink>>,
    #[serde(default)]
    event: HashMap<String, RawEvent>,
    #[serde(default)]
    narrative: HashMap<String, Narrative>,
}

fn main() -> ExitCode {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "../examples/story.yaml".to_string());
    let path = Path::new(&path);
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("读取 {path:?} 失败：{e}");
            return ExitCode::FAILURE;
        }
    };
    let base = path.parent().unwrap_or(Path::new("."));

    match check(&content, base) {
        Ok(summary) => {
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("解析失败：{e}");
            ExitCode::FAILURE
        }
    }
}

fn check(content: &str, base: &Path) -> Result<String, weft::errors::DocumentError> {
    let yaml: serde_yaml_ng::Value = serde_yaml_ng::from_str(content).map_err(weft::errors::DocumentError::raw)?;
    let raw: RawDoc = RawDoc::deserialize(&yaml).map_err(weft::errors::DocumentError::raw)?;

    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let gregorian = manifest.join("rune/gregorian.rn");

    let story = Story::from_raw(
        raw.story,
        StoryContext {
            base,
            gregorian: &gregorian,
            lang: "zh",
        },
    )?;
    let (entities, belong_dag) = weft::entity::build_entities(
        raw.entity,
        &raw.entity_link,
        weft::entity::EntityBuildContext {
            base,
            calendar: &story.calendar,
            calculable_props: &story.calculable_prop_runtime,
        },
    )?;
    event::check(&raw.event, &entities)?;
    narrative::check(&raw.narrative, &raw.event)?;
    let mut events = event::build_events(raw.event)?;
    event::apply_times(&mut events, &story.calendar)?;

    let roots = belong_dag
        .iter()
        .filter(|n| n.belong_to.is_empty())
        .count();
    Ok(format!(
        "解析成功：实体 {} 个（归属根 {} 个），关系组 {} 组，事件 {} 个，叙事线 {} 条",
        entities.len(),
        roots,
        raw.entity_link.len(),
        events.len(),
        raw.narrative.len(),
    ))
}
