use super::*;

#[test]
fn test_config_defaults() {
    let config = AppConfig::default();
    assert_eq!(config.font_size, 14);
    assert_eq!(config.theme, "system");
    assert_eq!(config.tab_size, 4);
    assert!(!config.word_wrap);
    assert!(config.llm_providers.is_empty());
    assert_eq!(config.squire_prefetch.memory_top_k, 10);
    assert_eq!(config.squire_prefetch.workflow_top_k, 3);
    assert_eq!(config.squire_prefetch.tool_top_k, 3);
    assert_eq!(config.squire_prefetch.skill_top_k, 3);
}

#[test]
fn test_toml_round_trip() {
    let config = AppConfig::default();
    let toml_str = toml::to_string_pretty(&config).unwrap();
    let parsed: AppConfig = toml::from_str(&toml_str).unwrap();
    assert_eq!(parsed.font_size, config.font_size);
    assert_eq!(parsed.tab_size, config.tab_size);
    assert_eq!(parsed.search_exclude.len(), 4);
    assert_eq!(parsed.terminal_font_size, 13);
    assert_eq!(parsed.squire_prefetch.memory_top_k, 10);
}

#[test]
fn test_provider_config() {
    let config = AppConfig {
        llm_providers: vec![ProviderConfig {
            provider_type: "openai".into(),
            name: "openai-main".into(),
            api_key: "sk-xxx".into(),
            model: "gpt-4".into(),
            phase2_model: String::new(),
            models: vec!["gpt-4".into()],
            endpoint: None,
            metadata: std::collections::HashMap::new(),
            category: None,
        }],
        ..Default::default()
    };
    assert_eq!(config.llm_providers.len(), 1);
    assert_eq!(config.llm_providers[0].model, "gpt-4");
}

#[test]
fn test_save_and_load() {
    let dir = std::env::temp_dir().join("squirecli-test-config-2");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    set_config_dir(dir.clone());

    let config = AppConfig::default();
    save_config(&config).unwrap();
    assert!(config_path().exists());

    let loaded = load_config().unwrap();
    assert_eq!(loaded.font_size, config.font_size);

    std::fs::remove_dir_all(dir).unwrap();
}
