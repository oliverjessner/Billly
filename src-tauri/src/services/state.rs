use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};

use tauri::{AppHandle, Emitter};

use crate::db::Database;
use crate::models::Settings;
use crate::services::processor::{mark_failed, process_invoice};
use crate::services::watcher::{debounce_file_event, FileEvent, FileEventKind, WatcherService};
use crate::utils::MAX_SCAN_DEPTH;

pub struct AppState {
    pub db: Arc<Mutex<Database>>,
    pub settings: Arc<Mutex<Settings>>,
    watcher: Mutex<Option<WatcherService>>,
}

impl AppState {
    pub fn new(db: Database, settings: Settings) -> Self {
        AppState {
            db: Arc::new(Mutex::new(db)),
            settings: Arc::new(Mutex::new(settings)),
            watcher: Mutex::new(None),
        }
    }

    pub fn update_settings(&self, settings: Settings, app: &AppHandle) -> Result<()> {
        {
            let mut locked = self.settings.lock().map_err(|_| anyhow!("Settings lock"))?;
            *locked = settings;
        }
        self.restart_watchers(app)
    }

    pub fn restart_watchers(&self, app: &AppHandle) -> Result<()> {
        let mut guard = self.watcher.lock().map_err(|_| anyhow!("Watcher lock"))?;
        *guard = None;

        let settings = self.settings.lock().map_err(|_| anyhow!("Settings lock"))?.clone();
        let (tx, rx) = mpsc::channel();
        let watcher = WatcherService::start(
            settings.revenue_folder.clone().map(PathBuf::from),
            settings.payable_folder.clone().map(PathBuf::from),
            tx,
        )?;

        *guard = Some(watcher);

        let db = self.db.clone();
        let settings_state = self.settings.clone();
        let app_handle = app.clone();
        std::thread::spawn(move || {
            for event in rx {
                handle_event(event, &db, &settings_state, &app_handle);
            }
        });

        Ok(())
    }

    pub fn enqueue_scan(&self, app: &AppHandle) -> Result<()> {
        let settings = self.settings.lock().map_err(|_| anyhow!("Settings lock"))?.clone();
        if let Some(folder) = settings.revenue_folder.clone() {
            self.scan_folder(PathBuf::from(folder), "revenue", app)?;
        }
        if let Some(folder) = settings.payable_folder.clone() {
            self.scan_folder(PathBuf::from(folder), "payable", app)?;
        }
        Ok(())
    }

    pub fn scan_folder(&self, folder: PathBuf, category: &str, app: &AppHandle) -> Result<()> {
        let entries = walkdir::WalkDir::new(folder)
            .max_depth(MAX_SCAN_DEPTH)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .filter(|e| is_pdf(e.path()))
            .map(|e| e.path().to_path_buf())
            .collect::<Vec<_>>();

        for path in entries {
            let db = self.db.clone();
            let settings = self.settings.clone();
            let app_handle = app.clone();
            let category = category.to_string();
            tauri::async_runtime::spawn(async move {
                let settings = match settings.lock() {
                    Ok(guard) => guard.clone(),
                    Err(_) => {
                        let _ = app_handle.emit("processing-error", "Settings lock".to_string());
                        return;
                    }
                };
                match process_invoice(&db, &path, &category, &settings, false).await {
                    Ok(invoice) => {
                        let _ = app_handle.emit("invoice-updated", invoice);
                    }
                    Err(err) => {
                        let invoice_opt = {
                            if let Ok(db_lock) = db.lock() {
                                db_lock.get_invoice_by_path(&path.to_string_lossy()).ok().flatten()
                            } else {
                                None
                            }
                        };
                        if let Some(mut invoice) = invoice_opt {
                            let _ = mark_failed(&db, &mut invoice, &format!("{:#}", err));
                            let _ = app_handle.emit("invoice-updated", invoice);
                        }
                        let _ = app_handle.emit("processing-error", format!("{:#}", err));
                    }
                }
            });
        }

        Ok(())
    }
}

fn handle_event(event: FileEvent, db: &Arc<Mutex<Database>>, settings: &Arc<Mutex<Settings>>, app: &AppHandle) {
    match event.kind {
        FileEventKind::Deleted => {
            if let Some(path_str) = event.path.to_str() {
                if let Ok(db) = db.lock() {
                    let _ = db.mark_invoice_missing(path_str);
                }
                let _ = app.emit("invoice-missing", path_str.to_string());
            }
        }
        _ => {
            if !debounce_file_event(&event.path, 700) {
                return;
            }

            let db = db.clone();
            let settings = settings.clone();
            let app_handle = app.clone();
            let category = event.category.clone();
            let path = event.path.clone();
            tauri::async_runtime::spawn(async move {
                let settings = match settings.lock() {
                    Ok(guard) => guard.clone(),
                    Err(_) => {
                        let _ = app_handle.emit("processing-error", "Settings lock".to_string());
                        return;
                    }
                };
                match process_invoice(&db, &path, &category, &settings, false).await {
                    Ok(invoice) => {
                        let _ = app_handle.emit("invoice-updated", invoice);
                    }
                    Err(err) => {
                        let invoice_opt = {
                            if let Ok(db_lock) = db.lock() {
                                db_lock.get_invoice_by_path(&path.to_string_lossy()).ok().flatten()
                            } else {
                                None
                            }
                        };
                        if let Some(mut invoice) = invoice_opt {
                            let _ = mark_failed(&db, &mut invoice, &format!("{:#}", err));
                            let _ = app_handle.emit("invoice-updated", invoice);
                        }
                        let _ = app_handle.emit("processing-error", format!("{:#}", err));
                    }
                }
            });
        }
    }
}

fn is_pdf(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false)
}
