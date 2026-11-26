use std::{env, fs, path::PathBuf};

fn is_non_empty_file(path: &PathBuf) -> bool {
    fs::metadata(path)
        .map(|m| m.is_file() && m.len() > 0)
        .unwrap_or(false)
}

fn ensure_dev_sidecar() {
    if env::var("PROFILE").as_deref() == Ok("release") {
        return;
    }

    let (Ok(target), Ok(manifest_dir)) = (env::var("TARGET"), env::var("CARGO_MANIFEST_DIR")) else {
        return;
    };

    let ext = if target.contains("windows") { ".exe" } else { "" };
    let manifest_path = PathBuf::from(&manifest_dir);
    let binaries_dir = manifest_path.join("binaries");
    let staged = binaries_dir.join(format!("weft-{target}{ext}"));
    if is_non_empty_file(&staged) {
        return;
    }

    if fs::create_dir_all(&binaries_dir).is_err() {
        return;
    }

    if let Some(workspace_root) = manifest_path.parent() {
        let built = workspace_root
            .join("target")
            .join("debug")
            .join(format!("weft{ext}"));
        if is_non_empty_file(&built) && fs::copy(&built, &staged).is_ok() {
            return;
        }
    }

    // 首次 debug check 或 tauri dev 尚未完成 beforeDevCommand 时创建空占位，
    // 避免 tauri-build 因缺少 externalBin 失败；真实二进制由 `yarn prepare:cli` 写入。
    if !staged.exists() {
        let _ = fs::write(&staged, []);
        println!(
            "cargo:warning=Created temporary placeholder {}; run `yarn prepare:cli --debug` to build the real weft CLI sidecar.",
            staged.display()
        );
    }
}

fn main() {
    ensure_dev_sidecar();
    tauri_build::build()
}
