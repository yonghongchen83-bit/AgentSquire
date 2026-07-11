use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex as TokioMutex;

use crate::agent::squire::SquireStore;
use crate::agent::{PendingApprovals, PendingAskUserQuestions};
use crate::fs::ops::FileEntry;
use crate::fs::watcher::FileWatcher;
use crate::llm::registry::{from_app_config_with_wire_log, ProviderInfo, ProviderRegistry};
use crate::mcp::DiscoveredTool;
use crate::search::grep::SearchMatch;
use crate::shell::exec::CommandResult;
use crate::state::config::{self, AppConfig, McpServerConfig};
use crate::storage::conversation_store::{ConversationStore, SessionSummary, SessionWithMessages};
use crate::terminal::manager::PtyManager;

pub mod config_update;
pub mod conversations;
pub mod diagnostics;
pub mod files;
pub mod git;
pub mod providers_cmd;
pub mod search;
pub mod setup_cmd;
pub mod shell;
pub mod stream_control;
pub mod streaming_cmd;
pub mod terminal_cmd;
pub mod tools_cmd;
pub mod utils;
pub mod watcher_cmd;
pub use diagnostics::{ErrorEntry, OutputEntry};

pub struct AppState {
    pub config: RwLock<AppConfig>,
    pub store: RwLock<Arc<dyn ConversationStore>>,
    pub registry: RwLock<ProviderRegistry>,
    pub stream_tasks: Arc<TokioMutex<HashMap<String, tokio::task::JoinHandle<()>>>>,
    pub subagent_tasks: Arc<TokioMutex<HashMap<String, tokio::task::JoinHandle<()>>>>,
    pub project_path: RwLock<String>,
    /// Squire context-mode memory store. Real LanceDB-backed implementation
    /// (`storage::squire_lancedb::LanceDbSquireStore`, Q4), constructed once
    /// at app startup in `setup_cmd.rs` against `<app_config_dir>/squire_lancedb`;
    /// persists across restarts. `agent::squire::InMemorySquireStore` remains
    /// available as a fast in-process test double for unit tests.
    ///
    /// Wrapped in `RwLock` so the active store can be swapped at runtime when
    /// binding/unbinding a workspace — the frontend session panel then refreshes
    /// to show only sessions scoped to that workspace.
    pub squire_store: RwLock<Arc<dyn SquireStore>>,
    /// Cache of discovered MCP tools per server ID. Populated on first
    /// discovery and reused across subsequent requests — MCP servers are
    /// long-lived processes whose tool list rarely changes within a session.
    /// Keyed by server ID so re-discovery is triggered automatically when
    /// the server config changes (app restart).
    /// Wrapped in Arc so it can be shared across tokio::spawn boundaries
    /// while allowing cache writes from within the spawned task.
    pub mcp_tools_cache: Arc<RwLock<HashMap<String, Vec<DiscoveredTool>>>>,
    /// Content hash of the last-ingested tool definitions. Used to skip
    /// `ingest_tool_registry` when nothing changed between turns.
    pub tool_registry_hash: Arc<RwLock<u64>>,
}

pub struct WatcherState {
    pub watcher: Mutex<FileWatcher>,
}

pub struct TerminalState {
    pub manager: PtyManager,
}

// ── Config ──

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    state
        .config
        .read()
        .map(|c| c.clone())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_config(new_config: AppConfig, state: State<'_, AppState>) -> Result<(), String> {
    config::save_config(&new_config).map_err(|e| e.to_string())?;
    *state.config.write().map_err(|e| e.to_string())? = new_config.clone();
    *state.registry.write().map_err(|e| e.to_string())? =
        crate::llm::registry::from_app_config(&new_config);
    Ok(())
}

#[tauri::command]
pub fn load_config() -> Result<AppConfig, String> {
    config_update::load_config_impl()
}

#[tauri::command]
pub fn check_update() -> Result<serde_json::Value, String> {
    Ok(config_update::check_update_impl())
}

// ── Project Path ──

#[tauri::command]
pub fn set_project_path(path: String, state: State<'_, AppState>) -> Result<(), String> {
    *state.project_path.write().map_err(|e| e.to_string())? = path;
    Ok(())
}

#[tauri::command]
pub fn get_project_path(state: State<'_, AppState>) -> Result<String, String> {
    state
        .project_path
        .read()
        .map(|p| p.clone())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_output(source: String) -> Result<Vec<OutputEntry>, String> {
    diagnostics::get_output_impl(source)
}

#[tauri::command]
pub fn get_errors() -> Result<Vec<ErrorEntry>, String> {
    diagnostics::get_errors_impl()
}

// ── Conversations ──

#[tauri::command]
pub async fn list_conversations(state: State<'_, AppState>) -> Result<Vec<SessionSummary>, String> {
    conversations::list_conversations_impl(state).await
}

#[tauri::command]
pub async fn get_conversation(
    state: State<'_, AppState>,
    id: String,
) -> Result<SessionWithMessages, String> {
    conversations::get_conversation_impl(state, id).await
}

#[tauri::command]
pub async fn create_conversation(
    state: State<'_, AppState>,
    title: String,
    context_mode: Option<String>,
) -> Result<crate::storage::conversation_store::Session, String> {
    conversations::create_conversation_impl(state, title, context_mode).await
}

#[tauri::command]
pub async fn delete_conversation(state: State<'_, AppState>, id: String) -> Result<(), String> {
    conversations::delete_conversation_impl(state, id).await
}

#[tauri::command]
pub async fn rename_conversation(
    state: State<'_, AppState>,
    id: String,
    title: String,
) -> Result<(), String> {
    conversations::rename_conversation_impl(state, id, title).await
}

#[tauri::command]
pub async fn truncate_messages_from(
    state: State<'_, AppState>,
    session_id: String,
    message_id: String,
) -> Result<(), String> {
    conversations::truncate_messages_from_impl(state, session_id, message_id).await
}

#[tauri::command]
pub async fn set_message_blocks(
    state: State<'_, AppState>,
    message_id: String,
    blocks_json: String,
) -> Result<(), String> {
    conversations::set_message_blocks_impl(state, message_id, blocks_json).await
}

// ── Workspace Binding ──

/// Bind to a workspace directory: swap the active stores to workspace-local
/// `.squire/squirecli.db` and `.squire/squire_lancedb/`. Emits
/// `workspace-changed` so the frontend refreshes the session panel.
#[tauri::command]
pub async fn bind_workspace(path: String, state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    let squire_dir = std::path::Path::new(&path).join(".squire");
    std::fs::create_dir_all(&squire_dir).map_err(|e| format!("Failed to create .squire directory: {}", e))?;

    // Open workspace-local SQLite store
    let db_path = squire_dir.join("squirecli.db");
    let db = crate::state::db::Database::open(&db_path)
        .map_err(|e| format!("Failed to open workspace database: {}", e))?;

    // Open workspace-local LanceDB store
    let lancedb_dir = squire_dir.join("squire_lancedb");
    let raw_store = squire_store::LanceDbSquireStore::open(&lancedb_dir)
        .await
        .map_err(|e| format!("Failed to open workspace Squire store: {}", e))?;
    let squire_store: Arc<dyn SquireStore> = Arc::new(raw_store);

    // Clear stale preserve lists on the new store
    squire_store.clear_all_preserve_lists().await;

    // ── Swap stores NOW so the frontend is usable immediately ──────────
    // Do NOT block the response on seeding workflows/skills — those
    // involve ONNX model loading + per-token embedding computation
    // (fastembed, first-call ~1-2s), and the user should see the
    // session panel refresh right away.
    *state.store.write().map_err(|e| e.to_string())? = Arc::new(db);
    *state.squire_store.write().map_err(|e| e.to_string())? = squire_store.clone();
    *state.project_path.write().map_err(|e| e.to_string())? = path.clone();

    // Redirect the provider wire log and Squire trace log under the
    // workspace's .squire/ directory
    let wire_log_path = Some(squire_dir.join("provider-wire.log"));
    squire_store::trace::set_trace_dir(squire_dir.clone());
    let config = state.config.read().map_err(|e| e.to_string())?.clone();
    let new_registry = from_app_config_with_wire_log(&config, wire_log_path);
    *state.registry.write().map_err(|e| e.to_string())? = new_registry;

    // Notify frontend to refresh session panel — this comes BEFORE the
    // expensive background seeding so the UI is responsive immediately.
    let _ = app.emit("workspace-changed", ());

    // ── Background seeding (workflows + skills) ────────────────────────
    // Seed in a fire-and-forget task so the Tauri command returns fast.
    // The Arc<dyn SquireStore> is shared, so background writes land in
    // the same store the active session reads from.
    let config_dir = crate::state::config::config_dir();
    let project_path = std::path::Path::new(&path).to_path_buf();
    tauri::async_runtime::spawn(async move {
        crate::agent::squire_workflows::seed_all_workflows_async(
            squire_store.clone(),
            &config_dir,
            Some(&project_path),
        ).await;
        crate::agent::squire_skills::seed_all_skills_async(
            squire_store.clone(),
            &config_dir,
            Some(&project_path),
        ).await;
    });

    Ok(())
}

/// Unbind the current workspace: revert to orphan-mode stores in the global
/// config directory. Emits `workspace-changed` so the frontend refreshes.
#[tauri::command]
pub async fn unbind_workspace(state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    let config_dir = crate::state::config::config_dir();

    // Open (or create) orphan-mode stores in config_dir
    let db_path = config_dir.join("squirecli.db");
    let db = crate::state::db::Database::open(&db_path)
        .map_err(|e| format!("Failed to open orphan database: {}", e))?;

    let lancedb_dir = config_dir.join("squire_lancedb");
    let raw_store = squire_store::LanceDbSquireStore::open(&lancedb_dir)
        .await
        .map_err(|e| format!("Failed to open orphan Squire store: {}", e))?;
    let squire_store: Arc<dyn SquireStore> = Arc::new(raw_store);

    squire_store.clear_all_preserve_lists().await;

    // Swap stores back to orphan mode
    *state.store.write().map_err(|e| e.to_string())? = Arc::new(db);
    *state.squire_store.write().map_err(|e| e.to_string())? = squire_store;
    *state.project_path.write().map_err(|e| e.to_string())? = String::new();

    // Revert the provider wire log and Squire trace log back to the
    // default config directory
    squire_store::trace::set_trace_dir(config_dir.clone());
    let config = state.config.read().map_err(|e| e.to_string())?.clone();
    let new_registry = crate::llm::registry::from_app_config(&config);
    *state.registry.write().map_err(|e| e.to_string())? = new_registry;

    let _ = app.emit("workspace-changed", ());

    Ok(())
}

// ── Send Message (with tool support) ──

#[tauri::command]
pub async fn send_message(
    app: AppHandle,
    state: State<'_, AppState>,
    pending_state: State<'_, PendingApprovals>,
    pending_ask_user_state: State<'_, PendingAskUserQuestions>,
    session_id: String,
    content: String,
    provider_name: Option<String>,
    model: Option<String>,
    thinking_level: Option<String>,
    phase2_provider: Option<String>,
    phase2_model: Option<String>,
) -> Result<(), String> {
    streaming_cmd::send_message_impl(
        app,
        state,
        pending_state,
        pending_ask_user_state,
        session_id,
        content,
        provider_name,
        model,
        thinking_level,
        phase2_provider,
        phase2_model,
    )
    .await
}

// ── Approve / Reject Tool Calls ──

#[tauri::command]
pub async fn abort_stream(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    stream_control::abort_stream_impl(app, state, session_id).await
}

#[tauri::command]
pub async fn abort_subagent(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    stream_control::abort_subagent_impl(app, state, session_id).await
}

#[tauri::command]
pub async fn approve_tool_call(
    pending_state: State<'_, PendingApprovals>,
    call_id: String,
) -> Result<(), String> {
    stream_control::approve_tool_call_impl(pending_state, call_id).await
}

#[tauri::command]
pub async fn reject_tool_call(
    pending_state: State<'_, PendingApprovals>,
    call_id: String,
) -> Result<(), String> {
    stream_control::reject_tool_call_impl(pending_state, call_id).await
}

// ── Answer AskUser Question (sa-5) ──

#[tauri::command]
pub async fn answer_ask_user_question(
    pending_state: State<'_, PendingAskUserQuestions>,
    question_id: String,
    answer: String,
) -> Result<(), String> {
    stream_control::answer_ask_user_question_impl(pending_state, question_id, answer).await
}

#[tauri::command]
pub async fn list_available_tools(
    state: State<'_, AppState>,
) -> Result<Vec<tools_cmd::ToolInfo>, String> {
    tools_cmd::list_available_tools(state).await
}

// ── LLM Providers ──

#[tauri::command]
pub fn list_providers(state: State<'_, AppState>) -> Vec<ProviderInfo> {
    providers_cmd::list_providers_impl(state)
}

#[tauri::command]
pub async fn test_connection(
    provider_type: String,
    api_key: String,
    model: String,
    endpoint: Option<String>,
) -> Result<String, String> {
    providers_cmd::test_connection_impl(provider_type, api_key, model, endpoint).await
}

#[tauri::command]
pub async fn fetch_models(
    provider_type: String,
    endpoint: String,
    api_key: Option<String>,
) -> Result<Vec<String>, String> {
    providers_cmd::fetch_models_impl(provider_type, endpoint, api_key).await
}

#[tauri::command]
pub async fn test_mcp_connection(server: McpServerConfig) -> Result<String, String> {
    providers_cmd::test_mcp_connection_impl(server).await
}

// ── File Operations ──

#[tauri::command]
pub fn read_file(path: String) -> Result<String, String> {
    files::read_file_impl(path)
}

#[tauri::command]
pub fn write_file(path: String, content: String) -> Result<(), String> {
    files::write_file_impl(path, content)
}

#[tauri::command]
pub fn create_dir(path: String) -> Result<(), String> {
    files::create_dir_impl(path)
}

#[tauri::command]
pub fn delete_item(path: String) -> Result<(), String> {
    files::delete_item_impl(path)
}

#[tauri::command]
pub fn rename_item(from: String, to: String) -> Result<(), String> {
    files::rename_item_impl(from, to)
}

#[tauri::command]
pub fn list_directory(path: String) -> Result<Vec<FileEntry>, String> {
    files::list_directory_impl(path)
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
    search::search_files_impl(
        query,
        path,
        regex,
        case_sensitive,
        whole_word,
        max_results,
        glob,
        context_lines,
    )
}

#[tauri::command]
pub fn replace_in_files(
    query: String,
    replacement: String,
    path: String,
    regex: bool,
    case_sensitive: bool,
    glob: Option<String>,
) -> Result<usize, String> {
    search::replace_in_files_impl(query, replacement, path, regex, case_sensitive, glob)
}

// ── Git ──

#[tauri::command]
pub fn git_status(path: Option<String>) -> Result<Vec<crate::fs::git::GitStatus>, String> {
    git::git_status_impl(path)
}

#[tauri::command]
pub fn git_diff(path: String, staged: bool) -> Result<Vec<crate::fs::git::GitDiff>, String> {
    git::git_diff_impl(path, staged)
}

#[tauri::command]
pub fn git_log(path: String, max_count: i32) -> Result<Vec<crate::fs::git::GitLogEntry>, String> {
    git::git_log_impl(path, max_count)
}

#[tauri::command]
pub fn git_branches(path: String) -> Result<Vec<crate::fs::git::GitBranch>, String> {
    git::git_branches_impl(path)
}

// ── Shell ──

#[tauri::command]
pub fn execute_command(
    command: String,
    args: Vec<String>,
    workdir: Option<String>,
) -> Result<CommandResult, String> {
    shell::execute_command_impl(command, args, workdir)
}

// ── File Watcher ──

#[tauri::command]
pub fn watch_directory(
    app: AppHandle,
    watcher_state: State<'_, WatcherState>,
    path: String,
) -> Result<(), String> {
    watcher_cmd::watch_directory_impl(&watcher_state.watcher, &path)?;
    let app_clone = app.clone();
    let _ = app_clone.emit("file-event", watcher_cmd::watch_started_event(path));

    Ok(())
}

// ── Terminal ──

#[tauri::command]
pub async fn spawn_terminal(
    app: AppHandle,
    term_state: State<'_, TerminalState>,
    shell: Option<String>,
) -> Result<String, String> {
    terminal_cmd::spawn_terminal_impl(&term_state.manager, app, shell).await
}

#[tauri::command]
pub async fn write_stdin(
    term_state: State<'_, TerminalState>,
    terminal_id: String,
    data: String,
) -> Result<(), String> {
    terminal_cmd::write_stdin_impl(&term_state.manager, terminal_id, data).await
}

#[tauri::command]
pub async fn resize_pty(
    term_state: State<'_, TerminalState>,
    terminal_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    terminal_cmd::resize_pty_impl(&term_state.manager, terminal_id, cols, rows).await
}

#[tauri::command]
pub async fn kill_terminal(
    term_state: State<'_, TerminalState>,
    terminal_id: String,
) -> Result<(), String> {
    terminal_cmd::kill_terminal_impl(&term_state.manager, terminal_id).await
}

#[tauri::command]
pub async fn list_terminals(term_state: State<'_, TerminalState>) -> Result<Vec<String>, String> {
    Ok(terminal_cmd::list_terminals_impl(&term_state.manager).await)
}

// ── App Setup ──

pub fn setup_app(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    setup_cmd::setup_app_impl(app)
}
