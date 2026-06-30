use crate::terminal::manager::PtyManager;
use tauri::AppHandle;

pub async fn spawn_terminal_impl(
    manager: &PtyManager,
    app: AppHandle,
    shell: Option<String>,
) -> Result<String, String> {
    manager.spawn(app, shell, None).await
}

pub async fn write_stdin_impl(
    manager: &PtyManager,
    terminal_id: String,
    data: String,
) -> Result<(), String> {
    manager.write(&terminal_id, &data).await
}

pub async fn resize_pty_impl(
    manager: &PtyManager,
    terminal_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    manager.resize(&terminal_id, cols, rows).await
}

pub async fn kill_terminal_impl(manager: &PtyManager, terminal_id: String) -> Result<(), String> {
    manager.kill(&terminal_id).await
}

pub async fn list_terminals_impl(manager: &PtyManager) -> Vec<String> {
    manager.list().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn list_terminals_impl_is_empty_for_new_manager() {
        let manager = PtyManager::new();
        let terms = list_terminals_impl(&manager).await;
        assert!(terms.is_empty());
    }

    #[tokio::test]
    async fn kill_terminal_impl_errors_for_unknown_terminal() {
        let manager = PtyManager::new();
        let result = kill_terminal_impl(&manager, "term-does-not-exist".to_string()).await;
        assert!(result.is_err());
    }
}
