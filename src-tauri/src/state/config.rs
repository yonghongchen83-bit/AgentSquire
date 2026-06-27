use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::OnceLock;

static CONFIG_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn set_config_dir(path: PathBuf) {
    let _ = CONFIG_DIR.set(path);
}

pub fn config_dir() -> PathBuf {
    CONFIG_DIR
        .get()
        .cloned()
        .unwrap_or_else(|| {
            dirs_fallback()
        })
}

fn dirs_fallback() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_default()
        .join(".squirecli")
}

fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub theme: ThemeConfig,
    pub llm: LlmConfig,
    pub editor: EditorConfig,
    pub search: SearchConfig,
    pub terminal: TerminalConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: ThemeConfig::default(),
            llm: LlmConfig::default(),
            editor: EditorConfig::default(),
            search: SearchConfig::default(),
            terminal: TerminalConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    pub mode: ThemeMode,
    pub font_size: u16,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            mode: ThemeMode::System,
            font_size: 14,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ThemeMode {
    Light,
    Dark,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub providers: Vec<ProviderConfig>,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            providers: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub name: String,
    pub provider_type: String,
    pub api_key: String,
    pub model: String,
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorConfig {
    pub tab_size: u8,
    pub word_wrap: bool,
    pub font_size: u16,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            tab_size: 4,
            word_wrap: false,
            font_size: 14,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    pub exclude_patterns: Vec<String>,
    pub max_results: usize,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            exclude_patterns: vec![
                "node_modules".into(),
                ".git".into(),
                "target".into(),
                "dist".into(),
            ],
            max_results: 1000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalConfig {
    pub shell_path: Option<String>,
    pub font_size: u16,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            shell_path: None,
            font_size: 13,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Failed to read config: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to parse config: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("Failed to serialize config: {0}")]
    Serialize(#[from] toml::ser::Error),
}

pub fn load_config() -> Result<AppConfig, ConfigError> {
    let path = config_path();
    if !path.exists() {
        let config = AppConfig::default();
        save_config(&config)?;
        return Ok(config);
    }
    let content = std::fs::read_to_string(&path)?;
    let config = toml::from_str(&content)?;
    Ok(config)
}

pub fn save_config(config: &AppConfig) -> Result<(), ConfigError> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir)?;
    let content = toml::to_string_pretty(config)?;
    std::fs::write(config_path(), content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = AppConfig::default();
        assert_eq!(config.theme.font_size, 14);
        assert!(matches!(config.theme.mode, ThemeMode::System));
        assert_eq!(config.editor.tab_size, 4);
        assert!(!config.editor.word_wrap);
        assert!(config.llm.providers.is_empty());
    }

    #[test]
    fn test_toml_round_trip() {
        let config = AppConfig::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: AppConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.theme.font_size, config.theme.font_size);
        assert_eq!(parsed.editor.tab_size, config.editor.tab_size);
        assert_eq!(parsed.search.max_results, 1000);
        assert_eq!(parsed.terminal.font_size, 13);
    }

    #[test]
    fn test_provider_config() {
        let config = AppConfig {
            llm: LlmConfig {
                providers: vec![
                    ProviderConfig {
                        name: "openai-main".into(),
                        provider_type: "openai".into(),
                        api_key: "sk-xxx".into(),
                        model: "gpt-4".into(),
                        base_url: None,
                    },
                ],
            },
            ..Default::default()
        };
        assert_eq!(config.llm.providers.len(), 1);
        assert_eq!(config.llm.providers[0].model, "gpt-4");
    }

    #[test]
    fn test_save_and_load() {
        let dir = std::env::temp_dir().join("squirecli-test-config");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        set_config_dir(dir.clone());

        let config = AppConfig::default();
        save_config(&config).unwrap();
        assert!(config_path().exists());

        let loaded = load_config().unwrap();
        assert_eq!(loaded.theme.font_size, config.theme.font_size);

        std::fs::remove_dir_all(dir).unwrap();
    }
}
