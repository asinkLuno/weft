//! 校验一份 `.weft` 故事文档：跑与 weft_doc::parse 相同的完整编译管线，
//! 成功打印摘要，失败打印文档错误。用法：cargo run --example check_story -- <story.weft>

#![allow(dead_code)]

#[path = "../src/calculable_prop.rs"]
mod calculable_prop;
#[path = "../src/calendar.rs"]
mod calendar;
#[path = "../src/plugin_loader.rs"]
mod plugin_loader;
#[path = "../src/weft/mod.rs"]
mod weft;

use std::{fs, path::Path, process::ExitCode};

fn main() -> ExitCode {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "../examples/story.weft".to_string());
    let path = Path::new(&path);
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("读取 {path:?} 失败：{e}");
            return ExitCode::FAILURE;
        }
    };
    let base = path.parent().unwrap_or(Path::new("."));
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let gregorian = manifest.join("rune/gregorian.rn");

    match weft::compiler::compile_with_scripts(&content, base, &gregorian, "zh").1 {
        Ok(doc) => {
            let roots = doc
                .belong_dag
                .iter()
                .filter(|n| n.belong_to.is_empty())
                .count();
            println!(
                "解析成功：实体 {} 个（归属根 {} 个），关系组 {} 组，事件 {} 个，叙事线 {} 条",
                doc.entity.len(),
                roots,
                doc.entity_link.len(),
                doc.event.len(),
                doc.narrative.len(),
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("解析失败：{e}");
            ExitCode::FAILURE
        }
    }
}
