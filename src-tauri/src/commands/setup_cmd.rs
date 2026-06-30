use super::{AppState, TerminalState, WatcherState};
use crate::agent::PendingApprovals;
use crate::fs::watcher::FileWatcher;
use crate::llm::registry::ProviderRegistry;
use crate::state::config::{self, AppConfig};
use crate::terminal::manager::PtyManager;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use tauri::{Emitter, Manager};
use tokio::sync::Mutex as TokioMutex;

pub fn setup_app_impl(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle = app.handle().clone();

    let config_dir = app
        .path()
        .app_config_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    config::set_config_dir(config_dir.clone());

    let config: AppConfig = config::load_config().unwrap_or_default();

    let db_path = config_dir.join("squirecli.db");
    let db = crate::state::db::Database::open(&db_path)
        .map_err(|e| format!("Failed to open database: {}", e))?;

    let registry = ProviderRegistry::from_config(&config);

    let (file_watcher, mut watcher_rx) = FileWatcher::new();

    tauri::async_runtime::spawn(async move {
        while let Ok(event) = watcher_rx.recv().await {
            let _ = app_handle.emit("file-event", event);
        }
    });

    app.manage(AppState {
        config: RwLock::new(config),
        store: Arc::new(db),
        registry: RwLock::new(registry),
        stream_tasks: Arc::new(TokioMutex::new(HashMap::new())),
    });

    app.manage(WatcherState {
        watcher: Mutex::new(file_watcher),
    });

    app.manage(TerminalState {
        manager: PtyManager::new(),
    });

    app.manage(PendingApprovals::new());

    if cfg!(debug_assertions) {
        app.handle().plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )?;
    }

    Ok(())
}
