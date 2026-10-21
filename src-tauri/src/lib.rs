mod document;

use std::sync::Mutex;
use tauri::menu::{Menu, SubmenuBuilder};
use tauri::Emitter;
use tauri_plugin_store::StoreExt;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .manage(Mutex::new(document::DocumentState::default()))
        .invoke_handler(tauri::generate_handler![
            document::bind_document,
            document::save_document,
            document::unbind_document,
        ])
        .setup(|app| {
            let settings = SubmenuBuilder::new(app, "设置")
                .text("swap-panes", "调换面板")
                .build()?;
            let menu = Menu::default(app.handle())?;
            menu.append(&settings)?;
            app.set_menu(menu)?;
            Ok(())
        })
        .on_menu_event(|app, event| {
            if event.id().0.as_str() != "swap-panes" {
                return;
            }
            let Ok(store) = app.store("settings.json") else {
                return;
            };
            let next = store
                .get("swapped")
                .and_then(|v| v.as_bool())
                .map(|b| !b)
                .unwrap_or(true);
            store.set("swapped", next);
            let _ = store.save();
            let _ = app.emit("settings-changed", next);
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
