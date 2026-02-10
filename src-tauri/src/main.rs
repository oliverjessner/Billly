#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod db;
mod models;
mod services;
mod utils;

use anyhow::anyhow;
use tauri::Manager;

use crate::models::Settings;
use crate::services::state::AppState;

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| anyhow!("App data dir: {}", e))?;
            std::fs::create_dir_all(&app_data_dir)?;

            let db_path = app_data_dir.join("billly.sqlite");
            let db = db::Database::new(db_path)?;
            let settings = load_settings(&db);

            let state = AppState::new(db, settings);
            state.restart_watchers(app.handle())?;
            state.enqueue_scan(app.handle())?;
            app.manage(state);

            #[cfg(target_os = "macos")]
            {
                use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder};
                use tauri::{WebviewUrl, WebviewWindowBuilder};

                let handle = app.handle();
                let about_item = MenuItemBuilder::with_id("about", "About Billly").build(app)?;

                let app_menu = SubmenuBuilder::new(handle, "Billly")
                    .item(&about_item)
                    .separator()
                    .item(&PredefinedMenuItem::services(handle, None)?)
                    .separator()
                    .item(&PredefinedMenuItem::hide(handle, None)?)
                    .item(&PredefinedMenuItem::hide_others(handle, None)?)
                    .item(&PredefinedMenuItem::show_all(handle, None)?)
                    .separator()
                    .item(&PredefinedMenuItem::quit(handle, None)?)
                    .build()?;

                let edit_menu = SubmenuBuilder::new(handle, "Edit")
                    .item(&PredefinedMenuItem::undo(handle, None)?)
                    .item(&PredefinedMenuItem::redo(handle, None)?)
                    .separator()
                    .item(&PredefinedMenuItem::cut(handle, None)?)
                    .item(&PredefinedMenuItem::copy(handle, None)?)
                    .item(&PredefinedMenuItem::paste(handle, None)?)
                    .item(&PredefinedMenuItem::select_all(handle, None)?)
                    .build()?;

                let menu = MenuBuilder::new(handle)
                    .item(&app_menu)
                    .item(&edit_menu)
                    .build()?;
                app.set_menu(menu)?;

                app.on_menu_event(|app, event| {
                    if event.id() == "about" {
                        if let Some(window) = app.get_webview_window("about") {
                            let _ = window.show();
                            let _ = window.set_focus();
                            return;
                        }
                        let _ = WebviewWindowBuilder::new(
                            app,
                            "about",
                            WebviewUrl::App("about.html".into()),
                        )
                        .title("About Billly")
                        .inner_size(420.0, 240.0)
                        .resizable(false)
                        .maximizable(false)
                        .minimizable(false)
                        .center()
                        .build();
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::test_openai_key,
            commands::settings::test_openai_key_saved,
            commands::settings::reprocess_all,
            commands::settings::pick_folder,
            commands::settings::remove_openai_key,
            commands::dashboard::get_dashboard_stats,
            commands::invoices::get_invoices,
            commands::invoices::get_invoice_years,
            commands::invoices::get_invoice_detail,
            commands::invoices::update_invoice_field,
            commands::invoices::clear_overrides,
            commands::invoices::clear_override,
            commands::invoices::delete_invoice,
            commands::invoices::reprocess_invoice,
            commands::invoices::open_invoice_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn load_settings(db: &db::Database) -> Settings {
    let revenue_folder = db.get_setting("revenue_folder").ok().flatten();
    let payable_folder = db.get_setting("payable_folder").ok().flatten();
    let mut openai_api_key = db.get_setting("openai_api_key").ok().flatten();
    if matches!(openai_api_key.as_deref(), Some(value) if value.starts_with("keychain:")) {
        let _ = db.delete_setting("openai_api_key");
        openai_api_key = None;
    }
    let ocr_language = db
        .get_setting("ocr_language")
        .ok()
        .flatten()
        .unwrap_or_else(|| "deu".to_string());
    Settings {
        revenue_folder,
        payable_folder,
        openai_api_key,
        ocr_language,
    }
}
