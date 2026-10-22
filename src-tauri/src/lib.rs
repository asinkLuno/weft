mod calculable_tag;
mod calendar;
mod document;
mod model;
mod validation;

use serde_json::json;
use std::sync::Mutex;
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
    let settings = SubmenuBuilder::new(app, settings_label)
        .item(&language)
        .build()?;
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
        .manage(Mutex::new(document::DocumentState::default()))
        .invoke_handler(tauri::generate_handler![
            document::bind_document,
            document::save_document,
            document::unbind_document,
        ])
        .setup(|app| {
            let lang = current_lang(app.handle());
            app.set_menu(build_menu(app.handle(), &lang)?)?;
            Ok(())
        })
        .on_menu_event(|app, event| {
            let lang = match event.id().0.as_str() {
                "lang-zh" => "zh",
                "lang-en" => "en",
                _ => return,
            };
            apply_lang(app, lang);
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
