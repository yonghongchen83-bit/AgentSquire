use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::fs::ops::{self, FileEntry};
use crate::fs::watcher::{FileEvent, FileWatcher};
use crate::llm::provider::StreamEvent;
use crate::llm::registry::ProviderRegistry;
use crate::search::grep::{self, SearchMatch, SearchOptions};
use crate::shell::exec::{self, CommandResult};
use crate::state::config::{self, AppConfig};
use crate::storage::conversation_store::{
    ConversationStore, NewMessage, NewSession, SessionId, SessionSummary, SessionWithMessages,
};

pub struct AppState {
    pub config: AppConfig,
    pub store: Arc<dyn ConversationStore>,
    pub registry: Arc<ProviderRegistry>,
}

pub struct WatcherState {
    pub watcher: Mutex<FileWatcher>,
}

// ── Config ──

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    Ok(state.config.clone())
}

#[tauri::command]
pub fn save_config(new_config: AppConfig) -> Result<(), String> {
    config::save_config(&new_config).map_err(|e| e.to_string())
}

// ── Conversations ──

#[tauri::command]
pub async fn list_conversations(
    state: State<'_, AppState>,
) -> Result<Vec<SessionSummary>, String> {
    state.store.list_sessions().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_conversation(
    state: State<'_, AppState>,
    id: String,
) -> Result<SessionWithMessages, String> {
    let session_id =
        SessionId::parse_str(&id).map_err(|e| format!("Invalid session ID: {}", e))?;
    state
        .store
        .get_session(session_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_conversation(
    state: State<'_, AppState>,
    title: String,
) -> Result<crate::storage::conversation_store::Session, String> {
    state
        .store
        .create_session(NewSession { title })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_conversation(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let session_id =
        SessionId::parse_str(&id).map_err(|e| format!("Invalid session ID: {}", e))?;
    state
        .store
        .delete_session(session_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn send_message(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    content: String,
    provider_name: Option<String>,
) -> Result<(), String> {
    let sid =
        SessionId::parse_str(&session_id).map_err(|e| format!("Invalid session ID: {}", e))?;

    state
        .store
        .append_message(NewMessage {
            session_id: sid,
            role: crate::storage::conversation_store::MessageRole::User,
            content: content.clone(),
        })
        .await
        .map_err(|e| e.to_string())?;

    let provider_key = provider_name
        .clone()
        .or_else(|| state.registry.default_name().map(|s| s.to_string()))
        .ok_or_else(|| "No default LLM provider configured".to_string())?;

    let session = state
        .store
        .get_session(sid)
        .await
        .map_err(|e| e.to_string())?;

    let registry = state.registry.clone();
    let store = state.store.clone();
    let app_clone = app.clone();

    tokio::spawn(async move {
        let provider = match registry.get(&provider_key) {
            Some(p) => p,
            None => {
                let _ = app_clone.emit("stream-error", format!("Provider '{}' not found", provider_key));
                return;
            }
        };

        let messages: Vec<crate::llm::provider::ChatMessage> = session
            .messages
            .iter()
            .map(|m| crate::llm::provider::ChatMessage {
                role: match m.role {
                    crate::storage::conversation_store::MessageRole::User => {
                        crate::llm::provider::ChatRole::User
                    }
                    crate::storage::conversation_store::MessageRole::Assistant => {
                        crate::llm::provider::ChatRole::Assistant
                    }
                    crate::storage::conversation_store::MessageRole::System => {
                        crate::llm::provider::ChatRole::System
                    }
                },
                content: m.content.clone(),
                tool_call_id: None,
            })
            .collect();

        let request = crate::llm::provider::ChatRequest {
            model: String::new(),
            messages,
            tools: Vec::new(),
            temperature: None,
            max_tokens: None,
        };

        let mut full_response = String::new();
        let mut stream = match provider.chat(request).await {
            Ok(s) => s,
            Err(e) => {
                let _ = app_clone.emit("stream-error", e.to_string());
                return;
            }
        };

        while let Some(event) = stream.recv().await {
            match event {
                StreamEvent::Chunk(text) => {
                    full_response.push_str(&text);
                    let _ = app_clone.emit("stream-chunk", text);
                }
                StreamEvent::ToolCall(tc) => {
                    let _ = app_clone.emit("stream-tool-call", tc);
                }
                StreamEvent::Done(_reason) => {
                    let content = std::mem::take(&mut full_response);
                    let msg = NewMessage {
                        session_id: sid,
                        role: crate::storage::conversation_store::MessageRole::Assistant,
                        content,
                    };
                    let _ = store.append_message(msg).await;
                    let _ = app_clone.emit("stream-done", "");
                }
                StreamEvent::Error(err) => {
                    let _ = app_clone.emit("stream-error", err);
                }
            }
        }
    });

    Ok(())
}

// ── LLM Providers ──

#[tauri::command]
pub fn list_providers(state: State<'_, AppState>) -> Vec<(String, String)> {
    state.registry.list()
}

// ── File Operations ──

#[tauri::command]
pub fn cmd_read_file(path: String) -> Result<String, String> {
    ops::read_file(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_write_file(path: String, content: String) -> Result<(), String> {
    ops::write_file(&path, &content).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_create_directory(path: String) -> Result<(), String> {
    ops::create_dir(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_delete_item(path: String) -> Result<(), String> {
    ops::delete_item(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_rename_item(from: String, to: String) -> Result<(), String> {
    ops::rename_item(&from, &to).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_list_directory(path: String) -> Result<Vec<FileEntry>, String> {
    ops::list_directory(&path).map_err(|e| e.to_string())
}

// ── Search ──

#[tauri::command]
pub fn search_files(
    query: String,
    path: String,
    regex: bool,
    case_sensitive: bool,
    whole_word: bool,
    max_results: Option<usize>,
    glob: Option<String>,
    context_lines: Option<u64>,
) -> Result<Vec<SearchMatch>, String> {
    let options = SearchOptions {
        query,
        path,
        regex,
        case_sensitive,
        whole_word,
        max_results,
        glob,
        context_lines,
    };
    grep::search(&options).map_err(|e| e.to_string())
}

// ── Git ──

#[tauri::command]
pub fn git_status(path: String) -> Result<Vec<crate::fs::git::GitStatus>, String> {
    crate::fs::git::status(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn git_diff(path: String, staged: bool) -> Result<Vec<crate::fs::git::GitDiff>, String> {
    crate::fs::git::diff(&path, staged).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn git_log(
    path: String,
    max_count: i32,
) -> Result<Vec<crate::fs::git::GitLogEntry>, String> {
    crate::fs::git::log(&path, max_count).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn git_branches(path: String) -> Result<Vec<crate::fs::git::GitBranch>, String> {
    crate::fs::git::branches(&path).map_err(|e| e.to_string())
}

// ── Shell ──

#[tauri::command]
pub fn execute_command(
    command: String,
    args: Vec<String>,
    workdir: Option<String>,
) -> Result<CommandResult, String> {
    exec::execute(&command, &args, workdir.as_deref()).map_err(|e| e.to_string())
}

// ── File Watcher ──

#[tauri::command]
pub fn watch_directory(
    app: AppHandle,
    watcher_state: State<'_, WatcherState>,
    path: String,
) -> Result<(), String> {
    let mut watcher = watcher_state
        .watcher
        .lock()
        .map_err(|e| format!("Lock error: {}", e))?;
    watcher.watch(&path)?;

    let app_clone = app.clone();
    // We need the receiver. For simplicity, we get it from the watcher.
    // The watcher_rx is stored in the app's state
    drop(watcher);

    // Emit initial event to confirm
    let _ = app_clone.emit("file-event", FileEvent {
        kind: "watch-started".into(),
        paths: vec![path],
    });

    Ok(())
}

// ── App Setup ──

pub fn setup_app(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle = app.handle().clone();

    let config_dir = app
        .path()
        .app_config_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    config::set_config_dir(config_dir.clone());

    let config = config::load_config().unwrap_or_default();

    let db_path = config_dir.join("squirecli.db");
    let db = crate::state::db::Database::open(&db_path)
        .map_err(|e| format!("Failed to open database: {}", e))?;

    let registry = Arc::new(ProviderRegistry::from_config(&config.llm));

    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();

    let (file_watcher, mut watcher_rx) = FileWatcher::new();

    tokio::spawn(async move {
        while let Ok(event) = watcher_rx.recv().await {
            let _ = app_handle.emit("file-event", event);
        }
    });

    app.manage(AppState {
        config,
        store: Arc::new(db),
        registry,
    });

    app.manage(WatcherState {
        watcher: Mutex::new(file_watcher),
    });

    if cfg!(debug_assertions) {
        app.handle().plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )?;
    }

    Ok(())
}
