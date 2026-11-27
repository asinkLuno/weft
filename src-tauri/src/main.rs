// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(unix)]
mod cli_install;
mod weft_doc;

use serde_json::json;
use std::sync::Mutex;
#[cfg(target_os = "linux")]
use tauri::Manager;
#[cfg(unix)]
use tauri::menu::MenuItem;
use tauri::menu::{CheckMenuItem, Menu, SubmenuBuilder};
use tauri::{AppHandle, Emitter, Theme, Wry};
use tauri_plugin_log::{RotationStrategy, Target, TargetKind, log::LevelFilter};
use tauri_plugin_store::StoreExt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ThemeMode {
    System,
    Light,
    Dark,
}

impl ThemeMode {
    fn from_str(value: &str) -> Option<Self> {
        match value {
            "system" => Some(Self::System),
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

// 菜单无法就地改文字，切语言或主题时整体重建
fn build_menu(app: &AppHandle, lang: &str, theme: ThemeMode) -> tauri::Result<Menu<Wry>> {
    let zh = lang == "zh";
    let (
        settings_label,
        language_label,
        appearance_label,
        theme_system_label,
        theme_light_label,
        theme_dark_label,
    ) = if zh {
        ("设置", "语言", "外观", "跟随系统", "浅色", "深色")
    } else {
        (
            "Settings",
            "Language",
            "Appearance",
            "System",
            "Light",
            "Dark",
        )
    };
    let item_zh = CheckMenuItem::with_id(app, "lang-zh", "中文", true, zh, None::<&str>)?;
    let item_en = CheckMenuItem::with_id(app, "lang-en", "English", true, !zh, None::<&str>)?;
    let language = SubmenuBuilder::new(app, language_label)
        .item(&item_zh)
        .item(&item_en)
        .build()?;

    let item_theme_system = CheckMenuItem::with_id(
        app,
        "theme-system",
        theme_system_label,
        true,
        theme == ThemeMode::System,
        None::<&str>,
    )?;
    let item_theme_light = CheckMenuItem::with_id(
        app,
        "theme-light",
        theme_light_label,
        true,
        theme == ThemeMode::Light,
        None::<&str>,
    )?;
    let item_theme_dark = CheckMenuItem::with_id(
        app,
        "theme-dark",
        theme_dark_label,
        true,
        theme == ThemeMode::Dark,
        None::<&str>,
    )?;
    let appearance = SubmenuBuilder::new(app, appearance_label)
        .item(&item_theme_system)
        .item(&item_theme_light)
        .item(&item_theme_dark)
        .build()?;

    let mut settings_builder = SubmenuBuilder::new(app, settings_label)
        .item(&language)
        .item(&appearance);
    #[cfg(unix)]
    if cli_install::should_show_menu() {
        let install_cli_label = if zh {
            "安装 weft 命令行工具…"
        } else {
            "Install 'weft' CLI in PATH…"
        };
        let item_install_cli =
            MenuItem::with_id(app, "install-cli", install_cli_label, true, None::<&str>)?;
        settings_builder = settings_builder.separator().item(&item_install_cli);
    }
    let settings = settings_builder.build()?;
    let menu = Menu::default(app)?;
    menu.append(&settings)?;
    Ok(menu)
}

// 已保存的语言优先，否则跟系统 locale
fn current_lang(app: &AppHandle) -> String {
    let saved = app
        .store("settings.json")
        .ok()
        .and_then(|s| s.get("language"))
        .and_then(|v| v.as_str().map(String::from));
    saved.unwrap_or_else(|| {
        sys_locale::get_locale()
            .map(|l| if l.starts_with("zh") { "zh" } else { "en" })
            .unwrap_or("en")
            .into()
    })
}

fn current_theme(app: &AppHandle) -> ThemeMode {
    app.store("settings.json")
        .ok()
        .and_then(|s| s.get("theme"))
        .and_then(|v| v.as_str().and_then(ThemeMode::from_str))
        .unwrap_or(ThemeMode::System)
}

fn apply_native_theme(app: &AppHandle, mode: ThemeMode) {
    match mode {
        ThemeMode::Light => app.set_theme(Some(Theme::Light)),
        ThemeMode::Dark => app.set_theme(Some(Theme::Dark)),
        ThemeMode::System => {
            #[cfg(target_os = "linux")]
            {
                // tao 在 Linux 上收到 SetTheme(None) 会强制把 GTK prefer-dark 设为 false；
                // 先清掉窗口覆盖再读 xdg-desktop-portal 的实际系统主题，以保持原生栏与 prefers-color-scheme 同步。
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.set_theme(None);
                    let os_theme = window.theme().unwrap_or(Theme::Light);
                    app.set_theme(Some(os_theme));
                } else {
                    app.set_theme(None);
                }
            }
            #[cfg(not(target_os = "linux"))]
            app.set_theme(None);
        }
    }
}

fn apply_lang(app: &AppHandle, lang: &str) {
    if let Ok(store) = app.store("settings.json") {
        store.set("language", lang);
        let _ = store.save();
    }
    let theme = current_theme(app);
    if let Ok(menu) = build_menu(app, lang, theme) {
        let _ = app.set_menu(menu);
    }
    let _ = app.emit("settings-changed", json!({ "language": lang }));
}

fn apply_theme(app: &AppHandle, theme: ThemeMode) {
    if let Ok(store) = app.store("settings.json") {
        store.set("theme", theme.as_str());
        let _ = store.save();
    }
    apply_native_theme(app, theme);
    let lang = current_lang(app);
    if let Ok(menu) = build_menu(app, &lang, theme) {
        let _ = app.set_menu(menu);
    }
    let _ = app.emit("settings-changed", json!({ "theme": theme.as_str() }));
}

#[tauri::command]
fn get_language(app: AppHandle) -> String {
    current_lang(&app)
}

#[tauri::command]
fn get_theme(app: AppHandle) -> &'static str {
    current_theme(&app).as_str()
}

fn main() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(LevelFilter::Info)
                .max_file_size(1_000_000)
                .rotation_strategy(RotationStrategy::KeepSome(3))
                .target(Target::new(TargetKind::Webview))
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .manage(Mutex::new(weft_doc::DocumentState::default()))
        .invoke_handler(tauri::generate_handler![
            weft_doc::open_document,
            weft_doc::compile_document,
            weft_doc::check_document,
            weft_doc::inspect_disk,
            weft_doc::reload_document,
            weft_doc::save_document,
            weft_doc::calendar_axis_ticks,
            weft_doc::close_document,
            get_language,
            get_theme,
        ])
        .setup(|app| {
            let lang = current_lang(app.handle());
            let theme = current_theme(app.handle());
            apply_native_theme(app.handle(), theme);
            app.set_menu(build_menu(app.handle(), &lang, theme)?)?;
            Ok(())
        })
        .on_menu_event(|app, event| match event.id().0.as_str() {
            "lang-zh" => apply_lang(app, "zh"),
            "lang-en" => apply_lang(app, "en"),
            "theme-system" => apply_theme(app, ThemeMode::System),
            "theme-light" => apply_theme(app, ThemeMode::Light),
            "theme-dark" => apply_theme(app, ThemeMode::Dark),
            #[cfg(unix)]
            "install-cli" => {
                let lang = current_lang(app);
                cli_install::trigger_install(app.clone(), lang);
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
