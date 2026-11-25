//! 输入文件发现：支持 `-`（stdin）、显式路径与尊重 `.gitignore` 的目录递归扫描。

use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

use ignore::WalkBuilder;

/// 待处理的单个 `.weft` 源码单元。
pub struct InputSource {
    pub display_name: String,
    pub base_dir: PathBuf,
    pub content: String,
}

impl InputSource {
    pub fn from_stdin() -> io::Result<Self> {
        let mut content = String::new();
        io::stdin().read_to_string(&mut content)?;
        let base_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Ok(Self {
            display_name: "<stdin>".to_owned(),
            base_dir,
            content,
        })
    }

    pub fn from_path(path: PathBuf) -> io::Result<Self> {
        let content = fs::read_to_string(&path)?;
        let display_name = path.display().to_string();
        let base_dir = match path.parent() {
            Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
            _ => PathBuf::from("."),
        };
        Ok(Self {
            display_name,
            base_dir,
            content,
        })
    }
}

/// 展开 CLI 传入的路径列表为有序 `.weft` 文件列表。
pub fn collect_weft_paths(inputs: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    let targets: Vec<PathBuf> = if inputs.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        inputs.to_vec()
    };

    for target in targets {
        if !target.exists() {
            return Err(format!("path does not exist: {}", target.display()));
        }
        if target.is_file() {
            files.push(target);
            continue;
        }
        if target.is_dir() {
            for entry in WalkBuilder::new(&target).build() {
                let entry = entry.map_err(|e| format!("failed to walk {}: {e}", target.display()))?;
                let path = entry.path();
                if path.is_file() && path.extension().is_some_and(|ext| ext == "weft") {
                    files.push(path.to_path_buf());
                }
            }
        }
    }

    files.sort();
    files.dedup();
    Ok(files)
}

pub fn is_stdin_arg(paths: &[PathBuf]) -> bool {
    paths.len() == 1 && paths[0].as_os_str() == "-"
}

pub fn format_rel_path(path: &Path) -> String {
    path.strip_prefix("./")
        .unwrap_or(path)
        .display()
        .to_string()
}
