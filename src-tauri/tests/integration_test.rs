use squirecli_lib::state::config::{AppConfig, ProviderConfig};
use squirecli_lib::storage::conversation_store::{ConversationStore, SessionId};

#[test]
fn test_modules_accessible() {
    let _ = squirecli_lib::commands::get_config;
    let _ = squirecli_lib::commands::list_conversations;
    let _ = squirecli_lib::commands::read_file;
    let _ = squirecli_lib::commands::search_files;
    let _ = squirecli_lib::commands::execute_command;
    let _ = squirecli_lib::commands::git_status;
    let _ = squirecli_lib::commands::approve_tool_call;
    let _ = squirecli_lib::commands::reject_tool_call;
}

#[test]
fn test_config_serde_roundtrip() {
    let config = AppConfig {
        theme: "dark".into(),
        font_size: 14,
        tab_size: 2,
        word_wrap: true,
        llm_providers: vec![
            ProviderConfig {
                provider_type: "openai".into(),
                name: "test".into(),
                api_key: String::new(),
                model: "gpt-4".into(),
                models: vec!["gpt-4".into()],
                endpoint: None,
                metadata: std::collections::HashMap::new(),
                category: None,
            },
        ],
        mcp_servers: vec![],
        search_exclude: vec!["node_modules".into()],
        terminal_shell: Some("powershell.exe".into()),
        terminal_font_size: 12,
        verbose_logging: false,
        left_panel_width: None,
        right_panel_width: None,
        bottom_panel_height: None,
        disabled_tools: vec![],
        squire_prefetch: Default::default(),
    };

    let json = serde_json::to_string(&config).unwrap();
    let deserialized: AppConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.theme, "dark");
    assert_eq!(deserialized.llm_providers.len(), 1);
    assert_eq!(deserialized.terminal_shell, Some("powershell.exe".into()));
}

#[test]
fn test_conversation_store_trait_is_object_safe() {
    fn _use_trait(_: &dyn ConversationStore) {}
    let _ = SessionId::new_v4();
}

#[test]
fn test_default_config_is_valid() {
    let config = AppConfig::default();
    assert_eq!(config.theme, "system");
    assert_eq!(config.font_size, 14);
    assert!(config.search_exclude.contains(&"node_modules".to_string()));
}
