#[cfg(unix)]
mod cli_install;
mod weft_doc;

use serde_json::json;
use std::sync::Mutex;
#[cfg(unix)]
use tauri::menu::MenuItem;
use tauri::menu::{CheckMenuItem, Menu, SubmenuBuilder};
use tauri::{AppHandle, Emitter, Wry};
use tauri_plugin_log::{RotationStrategy, Target, TargetKind, log::LevelFilter};
use tauri_plugin_store::StoreExt;

// 菜单无法就地改文字，切语言时整体重建
fn build_menu(app: &AppHandle, lang: &str) -> tauri::Result<Menu<Wry>> {
    let zh = lang == "zh";
    let (settings_label, language_label) = if zh {
        ("设置", "语言")
    } else {
        ("Settings", "Language")
    };
    let item_zh = CheckMenuItem::with_id(app, "lang-zh", "中文", true, zh, None::<&str>)?;
    let item_en = CheckMenuItem::with_id(app, "lang-en", "English", true, !zh, None::<&str>)?;
    let language = SubmenuBuilder::new(app, language_label)
        .item(&item_zh)
        .item(&item_en)
        .build()?;
    let mut settings_builder = SubmenuBuilder::new(app, settings_label).item(&language);
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

fn apply_lang(app: &AppHandle, lang: &str) {
    if let Ok(store) = app.store("settings.json") {
        store.set("language", lang);
        let _ = store.save();
    }
    if let Ok(menu) = build_menu(app, lang) {
        let _ = app.set_menu(menu);
    }
    let _ = app.emit("settings-changed", json!({ "language": lang }));
}

#[tauri::command]
fn get_language(app: AppHandle) -> String {
    current_lang(&app)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(LevelFilter::Info)
                .max_file_size(1_000_000)
                .rotation_strategy(RotationStrategy::KeepSome(3))
                .target(Target::new(TargetKind::Webview))
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
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
        ])
        .setup(|app| {
            let lang = current_lang(app.handle());
            app.set_menu(build_menu(app.handle(), &lang)?)?;
            Ok(())
        })
        .on_menu_event(|app, event| match event.id().0.as_str() {
            "lang-zh" => apply_lang(app, "zh"),
            "lang-en" => apply_lang(app, "en"),
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
