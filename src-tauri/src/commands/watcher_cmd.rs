use crate::fs::watcher::{FileEvent, FileWatcher};
use std::sync::Mutex;

pub fn watch_started_event(path: String) -> FileEvent {
    FileEvent {
        kind: "watch-started".into(),
        paths: vec![path],
    }
}

pub fn watch_directory_impl(watcher: &Mutex<FileWatcher>, path: &str) -> Result<(), String> {
    let mut watcher = watcher
        .lock()
        .map_err(|e| format!("Lock error: {}", e))?;
    watcher.watch(path)
}
