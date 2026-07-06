use super::{AppState, TerminalState, WatcherState};
use crate::agent::{PendingApprovals, PendingAskUserQuestions};
use crate::fs::watcher::FileWatcher;
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

    // Squire context-mode memory store (Q4): real LanceDB-backed store,
    // replacing the in-process InMemorySquireStore stand-in. `open` is
    // async (LanceDB's Connection::table_names/create_empty_table are);
    // `setup_app_impl` runs inside Tauri's sync `.setup()` closure, so we
    // block on Tauri's own async runtime here rather than making the whole
    // setup path async (matches Tauri v2's documented pattern for this).
    let squire_lancedb_dir = config_dir.join("squire_lancedb");
    let squire_store: std::sync::Arc<dyn crate::agent::squire::SquireStore> =
        std::sync::Arc::new(
            tauri::async_runtime::block_on(squire_store::LanceDbSquireStore::open(
                &squire_lancedb_dir,
            ))
            .map_err(|e| format!("Failed to open Squire LanceDB store: {}", e))?,
        );
    // Q7: preserve lists are a strict next-turn-only handoff, not long-lived
    // continuity state — clear any carryover left over from a previous app
    // run before any session can read it back as if it were still valid.
    tauri::async_runtime::block_on(squire_store.clear_all_preserve_lists());

    // ── Seed workflows from built-in, user, and project sources ──
    let initial_project_path = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let project_path_for_wf = if initial_project_path.is_empty()
        || initial_project_path == "."
    {
        None
    } else {
        Some(std::path::Path::new(&initial_project_path))
    };
    crate::agent::squire_workflows::seed_all_workflows(
        squire_store.clone(),
        &config_dir,
        project_path_for_wf,
    );
    // ─────────────────────────────────────────────────────────────

    // ── Seed skills from built-in, user, and project sources ──
    crate::agent::squire_skills::seed_all_skills(
        squire_store.clone(),
        &config_dir,
        project_path_for_wf,
    );
    // ─────────────────────────────────────────────────────────────

    let registry = crate::llm::registry::from_app_config(&config);

    let (file_watcher, mut watcher_rx) = FileWatcher::new();

    tauri::async_runtime::spawn(async move {
        while let Ok(event) = watcher_rx.recv().await {
            let _ = app_handle.emit("file-event", event);
        }
    });

    // Start background file watcher for workflow directory re-ingest.
    let project_path_for_watcher = if initial_project_path.is_empty()
        || initial_project_path == "."
    {
        None
    } else {
        Some(std::path::Path::new(&initial_project_path))
    };
    crate::agent::squire_workflows::start_workflow_watcher(
        squire_store.clone(),
        &config_dir,
        project_path_for_watcher,
    );

    // Start background file watcher for skill directory re-ingest.
    crate::agent::squire_skills::start_skill_watcher(
        squire_store.clone(),
        &config_dir,
        project_path_for_watcher,
    );

    app.manage(AppState {
        config: RwLock::new(config),
        store: Arc::new(db),
        registry: RwLock::new(registry),
        stream_tasks: Arc::new(TokioMutex::new(HashMap::new())),
        subagent_tasks: Arc::new(TokioMutex::new(HashMap::new())),
        project_path: RwLock::new(initial_project_path),
        squire_store,
    });

    app.manage(WatcherState {
        watcher: Mutex::new(file_watcher),
    });

    app.manage(TerminalState {
        manager: PtyManager::new(),
    });

    app.manage(PendingApprovals::new());
    app.manage(PendingAskUserQuestions::new());

    if cfg!(debug_assertions) {
        app.handle().plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )?;
    }

    Ok(())
}
