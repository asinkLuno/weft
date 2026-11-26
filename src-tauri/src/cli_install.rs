use std::{
    env, fs, io,
    os::unix::fs as unix_fs,
    path::{Path, PathBuf},
};
use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tauri_plugin_log::log::error;

#[cfg(target_os = "linux")]
use std::os::unix::fs::PermissionsExt;
#[cfg(target_os = "macos")]
use std::process::Command;

#[cfg(target_os = "macos")]
const MACOS_BIN_DIR: &str = "/usr/local/bin";
#[cfg(target_os = "macos")]
const MACOS_TARGET: &str = "/usr/local/bin/weft";

enum InstallOutcome {
    Installed { target: PathBuf, in_path: bool },
    AlreadyInstalled { target: PathBuf },
    #[cfg(target_os = "macos")]
    Cancelled,
}

fn bundled_cli_path() -> Result<PathBuf, String> {
    let exe = env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe
        .parent()
        .ok_or_else(|| "cannot resolve application directory".to_string())?;
    Ok(dir.join("weft"))
}

fn find_bundled_cli() -> Result<PathBuf, String> {
    let bundled = bundled_cli_path()?;
    let meta = fs::metadata(&bundled)
        .map_err(|_| format!("bundled weft CLI not found at {}", bundled.display()))?;
    if !meta.is_file() || meta.len() == 0 {
        return Err(format!(
            "bundled weft CLI at {} is missing or empty",
            bundled.display()
        ));
    }
    Ok(bundled)
}

/// Linux .deb / .rpm 安装包已将 weft 直接装到 /usr/bin/weft，无需再展示手动安装入口。
pub fn should_show_menu() -> bool {
    #[cfg(target_os = "linux")]
    {
        !matches!(bundled_cli_path(), Ok(p) if p == Path::new("/usr/bin/weft"))
    }
    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

fn replace_symlink(src: &Path, dst: &Path) -> io::Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    if fs::symlink_metadata(dst).is_ok() {
        fs::remove_file(dst)?;
    }
    unix_fs::symlink(src, dst)
}

#[cfg(target_os = "macos")]
fn install_with_macos_elevation(src: &Path, dst: &Path) -> Result<InstallOutcome, String> {
    let escape_sh = |s: &str| s.replace('\'', "'\\''");
    let shell_cmd = format!(
        "mkdir -p '{}' && ln -sf '{}' '{}'",
        escape_sh(MACOS_BIN_DIR),
        escape_sh(&src.to_string_lossy()),
        escape_sh(&dst.to_string_lossy()),
    );
    let applescript_cmd = shell_cmd.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!("do shell script \"{applescript_cmd}\" with administrator privileges");

    let output = Command::new("osascript")
        .args(["-e", &script])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        return Ok(InstallOutcome::Installed {
            target: dst.to_path_buf(),
            in_path: true,
        });
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("-128") || stderr.to_lowercase().contains("user canceled") {
        return Ok(InstallOutcome::Cancelled);
    }
    Err(stderr.trim().to_string())
}

#[cfg(target_os = "macos")]
fn install_cli() -> Result<InstallOutcome, String> {
    let bundled = find_bundled_cli()?;
    let target = PathBuf::from(MACOS_TARGET);

    if fs::read_link(&target).map(|p| p == bundled).unwrap_or(false) {
        return Ok(InstallOutcome::AlreadyInstalled { target });
    }

    // /usr/local/bin 位于 macOS 默认 /etc/paths 中；GUI 进程从 Finder 启动时自身 PATH 不含该目录，
    // 因此直接视为在用户终端 PATH 内，避免误报提示。
    match replace_symlink(&bundled, &target) {
        Ok(()) => Ok(InstallOutcome::Installed {
            target,
            in_path: true,
        }),
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
            install_with_macos_elevation(&bundled, &target)
        }
        Err(err) => Err(err.to_string()),
    }
}

#[cfg(target_os = "linux")]
fn is_dir_in_path(dir: &Path) -> bool {
    let Some(path_var) = env::var_os("PATH") else {
        return false;
    };
    let canonical_dir = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    env::split_paths(&path_var).any(|entry| {
        entry == dir
            || fs::canonicalize(&entry)
                .map(|p| p == canonical_dir)
                .unwrap_or(false)
    })
}

#[cfg(target_os = "linux")]
fn copy_atomic_executable(src: &Path, dst: &Path) -> io::Result<()> {
    let parent = dst
        .parent()
        .ok_or_else(|| io::Error::other("invalid destination path"))?;
    fs::create_dir_all(parent)?;
    let tmp = parent.join(".weft-cli.tmp");
    fs::copy(src, &tmp)?;
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755))?;
    if let Err(err) = fs::rename(&tmp, dst) {
        let _ = fs::remove_file(&tmp);
        return Err(err);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn install_cli() -> Result<InstallOutcome, String> {
    let bundled = find_bundled_cli()?;
    let home =
        env::var_os("HOME").ok_or_else(|| "HOME environment variable is not set".to_string())?;
    let bin_dir = PathBuf::from(home).join(".local").join("bin");
    let target = bin_dir.join("weft");

    // AppImage 运行时的挂载目录退出即销毁，必须复制而不是软链
    if env::var_os("APPIMAGE").is_some() {
        copy_atomic_executable(&bundled, &target).map_err(|e| e.to_string())?;
    } else if fs::read_link(&target).map(|p| p == bundled).unwrap_or(false) {
        return Ok(InstallOutcome::AlreadyInstalled { target });
    } else {
        replace_symlink(&bundled, &target).map_err(|e| e.to_string())?;
    }

    Ok(InstallOutcome::Installed {
        target,
        in_path: is_dir_in_path(&bin_dir),
    })
}

pub fn trigger_install(app: AppHandle, lang: String) {
    std::thread::spawn(move || {
        let zh = lang == "zh";
        let title = if zh {
            "安装 weft 命令行工具"
        } else {
            "Install 'weft' CLI"
        };

        let (kind, msg) = match install_cli() {
            Ok(InstallOutcome::Installed { target, in_path }) => {
                let path_str = target.display();
                let dir_str = target
                    .parent()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                let text = match (zh, in_path) {
                    (true, true) => format!(
                        "已安装 `weft` 命令行工具到 {path_str}。\n现在可以直接在终端运行 `weft check` / `weft fmt`。"
                    ),
                    (true, false) => format!(
                        "已安装 `weft` 命令行工具到 {path_str}。\n请确保 {dir_str} 已加入 shell 的 PATH 环境变量。"
                    ),
                    (false, true) => format!(
                        "Installed `weft` CLI at {path_str}.\nYou can now run `weft check` / `weft fmt` from your terminal."
                    ),
                    (false, false) => format!(
                        "Installed `weft` CLI at {path_str}.\nMake sure {dir_str} is in your shell's PATH."
                    ),
                };
                (MessageDialogKind::Info, text)
            }
            Ok(InstallOutcome::AlreadyInstalled { target }) => {
                let path_str = target.display();
                let text = if zh {
                    format!("`weft` 命令行工具已就绪：{path_str}")
                } else {
                    format!("`weft` CLI is already installed at {path_str}.")
                };
                (MessageDialogKind::Info, text)
            }
            #[cfg(target_os = "macos")]
            Ok(InstallOutcome::Cancelled) => return,
            Err(err) => {
                error!("failed to install weft CLI: {err}");
                let text = if zh {
                    format!("安装 `weft` 命令行工具失败：\n{err}")
                } else {
                    format!("Failed to install `weft` CLI:\n{err}")
                };
                (MessageDialogKind::Error, text)
            }
        };

        app.dialog()
            .message(msg)
            .title(title)
            .kind(kind)
            .show(|_| {});
    });
}
