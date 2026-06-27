use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use tokio::sync::broadcast;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileEvent {
    pub kind: String,
    pub paths: Vec<String>,
}

pub struct FileWatcher {
    watcher: Option<RecommendedWatcher>,
    #[allow(dead_code)]
    tx: broadcast::Sender<FileEvent>,
}

impl FileWatcher {
    pub fn new() -> (Self, broadcast::Receiver<FileEvent>) {
        let (tx, rx) = broadcast::channel(256);

        let tx_clone = tx.clone();
        let watcher = RecommendedWatcher::new(
            move |event: Result<Event, notify::Error>| {
                if let Ok(event) = event {
                    let kind = match event.kind {
                        EventKind::Create(_) => "create",
                        EventKind::Modify(_) => "modify",
                        EventKind::Remove(_) => "delete",
                        _ => "other",
                    }
                    .to_string();

                    let paths: Vec<String> = event
                        .paths
                        .iter()
                        .map(|p| p.to_string_lossy().to_string())
                        .collect();

                    let fe = FileEvent { kind, paths };
                    tx_clone.send(fe).ok();
                }
            },
            Config::default(),
        )
        .ok();

        (
            Self { watcher, tx },
            rx,
        )
    }

    pub fn watch(&mut self, path: &str) -> Result<(), String> {
        let watcher = self
            .watcher
            .as_mut()
            .ok_or_else(|| "Watcher not initialized".to_string())?;
        watcher
            .watch(Path::new(path), RecursiveMode::Recursive)
            .map_err(|e| e.to_string())
    }

    pub fn unwatch(&mut self, path: &str) -> Result<(), String> {
        let watcher = self
            .watcher
            .as_mut()
            .ok_or_else(|| "Watcher not initialized".to_string())?;
        watcher
            .unwatch(Path::new(path))
            .map_err(|e| e.to_string())
    }
}
