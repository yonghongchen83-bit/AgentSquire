pub mod commands;
pub mod llm;
pub mod storage;
pub mod fs;
pub mod search;
pub mod state;
pub mod agent;
pub mod shell;

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
            commands::delete_conversation,
            commands::send_message,
            commands::list_providers,
            commands::cmd_read_file,
            commands::cmd_write_file,
            commands::cmd_create_directory,
            commands::cmd_delete_item,
            commands::cmd_rename_item,
            commands::cmd_list_directory,
            commands::search_files,
            commands::git_status,
            commands::git_diff,
            commands::git_log,
            commands::git_branches,
            commands::execute_command,
            commands::watch_directory,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
