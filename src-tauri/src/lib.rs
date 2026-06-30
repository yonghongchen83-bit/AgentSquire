pub mod agent;
pub mod commands;
pub mod fs;
pub mod llm;
pub mod mcp;
pub mod search;
pub mod shell;
pub mod state;
pub mod storage;
pub mod terminal;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            commands::setup_app(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_config,
            commands::list_conversations,
            commands::get_conversation,
            commands::create_conversation,
            commands::rename_conversation,
            commands::delete_conversation,
            commands::send_message,
            commands::abort_stream,
            commands::list_providers,
            commands::read_file,
            commands::write_file,
            commands::create_dir,
            commands::delete_item,
            commands::rename_item,
            commands::list_directory,
            commands::search_files,
            commands::replace_in_files,
            commands::git_status,
            commands::git_diff,
            commands::git_log,
            commands::git_branches,
            commands::execute_command,
            commands::watch_directory,
            commands::approve_tool_call,
            commands::reject_tool_call,
            commands::load_config,
            commands::check_update,
            commands::test_connection,
            commands::test_mcp_connection,
            commands::fetch_models,
            commands::spawn_terminal,
            commands::write_stdin,
            commands::resize_pty,
            commands::kill_terminal,
            commands::list_terminals,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
