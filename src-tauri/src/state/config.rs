use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::OnceLock;

static CONFIG_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn set_config_dir(path: PathBuf) {
    let _ = CONFIG_DIR.set(path);
}

pub fn config_dir() -> PathBuf {
    CONFIG_DIR.get().cloned().unwrap_or_else(dirs_fallback)
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
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub theme: String,
    pub font_size: u16,
    pub tab_size: u8,
    pub word_wrap: bool,
    pub llm_providers: Vec<ProviderConfig>,
    #[serde(default)]
    pub mcp_servers: Vec<McpServerConfig>,
    pub search_exclude: Vec<String>,
    pub terminal_shell: Option<String>,
    pub terminal_font_size: u16,
    pub verbose_logging: bool,
    pub left_panel_width: Option<f64>,
    pub right_panel_width: Option<f64>,
    pub bottom_panel_height: Option<f64>,
    #[serde(default)]
    pub disabled_tools: Vec<String>,
    #[serde(default)]
    pub squire_prefetch: SquirePrefetchConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SquirePrefetchConfig {
    pub memory_top_k: u32,
    pub workflow_top_k: u32,
    pub tool_top_k: u32,
    pub skill_top_k: u32,
}

impl Default for SquirePrefetchConfig {
    fn default() -> Self {
        Self {
            memory_top_k: 10,
            workflow_top_k: 3,
            tool_top_k: 3,
            skill_top_k: 3,
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            font_size: 14,
            tab_size: 4,
            word_wrap: false,
            llm_providers: Vec::new(),
            mcp_servers: Vec::new(),
            search_exclude: vec![
                "node_modules".into(),
                ".git".into(),
                "target".into(),
                "dist".into(),
            ],
            terminal_shell: None,
            terminal_font_size: 13,
            verbose_logging: false,
            left_panel_width: None,
            right_panel_width: None,
            bottom_panel_height: None,
            disabled_tools: Vec::new(),
            squire_prefetch: SquirePrefetchConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    #[serde(alias = "id")]
    pub provider_type: String,
    pub name: String,
    #[serde(default)]
    pub api_key: String,
    pub model: String,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub metadata: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct McpServerConfig {
    pub id: String,
    pub name: String,
    #[serde(default = "default_transport")]
    pub transport: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
}

// Conversion between the main crate's McpServerConfig and squire-store's
// McpServerConfig (identical shape, separate types to avoid a dependency
// edge from the store crate back to the main crate).
impl From<squire_store::McpServerConfig> for McpServerConfig {
    fn from(c: squire_store::McpServerConfig) -> Self {
        McpServerConfig {
            id: c.id,
            name: c.name,
            transport: c.transport,
            command: c.command,
            args: c.args,
            url: c.url,
            enabled: c.enabled,
            env: c.env,
            headers: c.headers,
        }
    }
}

impl From<McpServerConfig> for squire_store::McpServerConfig {
    fn from(c: McpServerConfig) -> Self {
        squire_store::McpServerConfig {
            id: c.id,
            name: c.name,
            transport: c.transport,
            command: c.command,
            args: c.args,
            url: c.url,
            enabled: c.enabled,
            env: c.env,
            headers: c.headers,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_transport() -> String {
    "stdio".to_string()
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
#[path = "config_test.rs"]
mod tests;
