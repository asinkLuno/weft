//! 用 examples/story.yaml 跑一遍实体构建，打印归属 DAG。
//! 与 weft_doc::WeftDoc::from_yaml 走同一条 Story → entity::build 管线，
//! 只是不经过 Tauri AppHandle（资产授权等运行时部分与本图无关）。

#![allow(dead_code)]

#[path = "../src/calculable_prop.rs"]
mod calculable_prop;
#[path = "../src/calendar.rs"]
mod calendar;
#[path = "../src/plugin_loader.rs"]
mod plugin_loader;
#[path = "../src/weft/mod.rs"]
mod weft;

use std::{collections::HashMap, fs, path::Path};

use serde::Deserialize;
use weft::FromRaw;
use weft::entity::{BelongNode, EntityBuildContext, EntityLink, RawEntity, build};
use weft::story::{RawStory, Story, StoryContext};

// 与 RawWeftDoc 的实体相关字段一致；其余段与归属 DAG 无关。
#[derive(Deserialize)]
struct RawDoc {
    story: RawStory,
    #[serde(default)]
    entity: HashMap<String, RawEntity>,
    #[serde(default)]
    entity_link: HashMap<String, Vec<EntityLink>>,
}

fn main() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let base = manifest.join("../examples");
    let gregorian = manifest.join("rhai/gregorian.rhai");
    let content = fs::read_to_string(base.join("story.yaml")).expect("read story.yaml");
    let raw: RawDoc =
        serde_yaml_ng::from_str(&content).expect("deserialize story.yaml");

    let date_mode = raw
        .story
        .date_mode
        .clone()
        .filter(|source| !source.trim().is_empty());
    let story = Story::from_raw(
        raw.story,
        StoryContext {
            base: &base,
            gregorian: &gregorian,
            lang: "zh",
        },
    )
    .expect("build story");
    let (entities, belong_dag) = build(
        raw.entity,
        &raw.entity_link,
        EntityBuildContext {
            base: &base,
            calendar: &story.calendar,
            calculable_props: &story.calculable_prop_runtime,
            date_mode: date_mode.as_deref(),
            lang: "zh",
        },
    )
    .expect("build entities");

    print_dag(&belong_dag);
    println!("\n实体数：{}", entities.len());
    println!(
        "\nbelong_dag JSON：\n{}",
        serde_json::to_string_pretty(&belong_dag).expect("serialize dag")
    );
}

// 边是 子 → 父（belong_to），按根向下展开；多重归属的节点会在每个父节点下重复出现。
fn print_dag(dag: &[BelongNode]) {
    let mut children: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut roots = Vec::new();
    for node in dag {
        if node.belong_to.is_empty() {
            roots.push(node.id.as_str());
        }
        for parent in &node.belong_to {
            children
                .entry(parent.as_str())
                .or_default()
                .push(node.id.as_str());
        }
    }
    roots.sort_unstable();
    for list in children.values_mut() {
        list.sort_unstable();
    }
    println!("归属 DAG（{} 个节点）：", dag.len());
    for root in roots {
        print_node(root, &children, "", true, true);
    }
}

fn print_node(
    id: &str,
    children: &HashMap<&str, Vec<&str>>,
    prefix: &str,
    last: bool,
    root: bool,
) {
    if root {
        println!("{id}");
    } else {
        println!("{prefix}{}{id}", if last { "└── " } else { "├── " });
    }
    let Some(kids) = children.get(id) else {
        return;
    };
    let child_prefix = if root {
        String::new()
    } else {
        format!("{prefix}{}", if last { "    " } else { "│   " })
    };
    for (index, child) in kids.iter().enumerate() {
        print_node(
            child,
            children,
            &child_prefix,
            index + 1 == kids.len(),
            false,
        );
    }
}
