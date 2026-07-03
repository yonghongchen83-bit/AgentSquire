use squirecli_lib::commands::terminal_cmd::{list_terminals_impl, kill_terminal_impl};
use squirecli_lib::terminal::manager::PtyManager;

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
