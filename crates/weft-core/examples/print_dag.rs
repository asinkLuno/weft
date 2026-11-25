//! 用 examples/story.weft 跑一遍完整编译，打印归属 DAG。

use std::{collections::HashMap, fs, path::Path};

use weft_core::{compiler, entity::BelongNode};

fn main() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let base = manifest.join("../../examples");
    let content = fs::read_to_string(base.join("story.weft")).expect("read story.weft");

    let doc = compiler::compile_with_scripts(&content, &base, "zh")
        .1
        .expect("compile story");

    print_dag(&doc.belong_dag);
    println!("\n实体数：{}", doc.entity.len());
    println!(
        "\nbelong_dag JSON：\n{}",
        serde_json::to_string_pretty(&doc.belong_dag).expect("serialize dag")
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
