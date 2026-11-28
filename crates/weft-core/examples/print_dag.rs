//! 用 examples/story.weft 跑一遍完整编译，打印关系图定义、边列表与等价类。

use std::{fs, path::Path};

use weft_core::{compiler, relation::equivalence_classes};

fn main() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let base = manifest.join("../../examples");
    let content = fs::read_to_string(base.join("story.weft")).expect("read story.weft");

    let doc = compiler::compile_with_scripts(&content, &base, "zh")
        .1
        .expect("compile story");

    println!("关系图数：{}", doc.graphs.len());
    for (name, spec) in &doc.graphs {
        println!("  {name}: {:?}", spec.enabled_properties());
    }
    println!("实体数：{}", doc.entity.len());
    println!("关系边数：{}", doc.links.len());

    let eq_classes = equivalence_classes(&doc.links, &doc.graphs);
    if !eq_classes.is_empty() {
        println!("等价类：{eq_classes:?}");
    }
}
