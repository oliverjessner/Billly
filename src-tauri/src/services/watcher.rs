use anyhow::Result;
use notify::{recommended_watcher, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use crate::utils::within_allowed_depth;

#[derive(Debug, Clone)]
pub enum FileEventKind {
    Created,
    Modified,
    Deleted,
}

#[derive(Debug, Clone)]
pub struct FileEvent {
    pub path: PathBuf,
    pub category: String,
    pub kind: FileEventKind,
}

pub struct WatcherService {
    _watchers: Vec<RecommendedWatcher>,
}

impl WatcherService {
    pub fn start(
        revenue_folder: Option<PathBuf>,
        payable_folder: Option<PathBuf>,
        tx: mpsc::Sender<FileEvent>,
    ) -> Result<Self> {
        let mut watchers = Vec::new();
        if let Some(path) = revenue_folder {
            if path.exists() {
                watchers.push(create_watcher(path, "revenue".to_string(), tx.clone())?);
            }
        }
        if let Some(path) = payable_folder {
            if path.exists() {
                watchers.push(create_watcher(path, "payable".to_string(), tx.clone())?);
            }
        }
        Ok(WatcherService { _watchers: watchers })
    }
}

fn create_watcher(
    base_path: PathBuf,
    category: String,
    tx: mpsc::Sender<FileEvent>,
) -> notify::Result<RecommendedWatcher> {
    let base_for_filter = base_path.clone();
    let mut watcher = recommended_watcher(move |res: notify::Result<Event>| {
        if let Ok(event) = res {
            let kind = match event.kind {
                EventKind::Create(_) => FileEventKind::Created,
                EventKind::Modify(_) => FileEventKind::Modified,
                EventKind::Remove(_) => FileEventKind::Deleted,
                _ => return,
            };
            for path in event.paths {
                if is_pdf(&path) && within_allowed_depth(&base_for_filter, &path) {
                    let _ = tx.send(FileEvent {
                        path: path.to_path_buf(),
                        category: category.clone(),
                        kind: kind.clone(),
                    });
                }
            }
        }
    })?;

    watcher.watch(&base_path, RecursiveMode::Recursive)?;
    Ok(watcher)
}

fn is_pdf(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false)
}

pub fn debounce_file_event(path: &Path, debounce_ms: u64) -> bool {
    let mut last_size = None;
    for _ in 0..3 {
        std::thread::sleep(Duration::from_millis(debounce_ms));
        if let Ok(metadata) = std::fs::metadata(path) {
            let size = metadata.len();
            if Some(size) == last_size {
                return size > 0;
            }
            last_size = Some(size);
        } else {
            return false;
        }
    }
    last_size.unwrap_or(0) > 0
}
