use std::sync::Arc;

use super::openai::OpenAIProvider;
use super::anthropic::AnthropicProvider;
use super::provider::LlmProvider;
use crate::state::config::AppConfig;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ProviderInfo {
    pub name: String,
    pub provider_type: String,
    pub models: Vec<String>,
    pub default_model: String,
}

#[derive(Clone)]
pub struct ProviderEntry {
    pub name: String,
    pub provider_type: String,
    pub provider: Arc<dyn LlmProvider>,
    pub models: Vec<String>,
    pub default_model: String,
    pub api_key: String,
    pub endpoint: Option<String>,
}

pub struct ProviderRegistry {
    entries: Vec<ProviderEntry>,
    default_index: Option<usize>,
}

impl ProviderRegistry {
    pub fn from_config(config: &AppConfig) -> Self {
        let mut entries = Vec::new();
        let mut default_index = None;

        for cfg in &config.llm_providers {
            let model_list: Vec<String> = if cfg.models.is_empty() {
                vec![cfg.model.clone()]
            } else {
                cfg.models.clone()
            };

            if model_list.is_empty() {
                continue;
            }

            let default_model = if cfg.model.is_empty() {
                model_list[0].clone()
            } else {
                cfg.model.clone()
            };

            let provider_type = cfg.provider_type.to_lowercase();

            let provider: Arc<dyn LlmProvider> = match provider_type.as_str() {
                "openai" => {
                    let mut p = OpenAIProvider::new(
                        cfg.api_key.clone(),
                        default_model.clone(),
                        cfg.endpoint.clone(),
                    );
                    p.verbose = config.verbose_logging;
                    Arc::new(p) as Arc<dyn LlmProvider>
                }
                "anthropic" => {
                    let mut p = AnthropicProvider::new(
                        cfg.api_key.clone(),
                        default_model.clone(),
                        cfg.endpoint.clone(),
                    );
                    p.verbose = config.verbose_logging;
                    Arc::new(p) as Arc<dyn LlmProvider>
                }
                _ => {
                    tracing::warn!("Unknown LLM provider type: {}", provider_type);
                    continue;
                }
            };

            if default_index.is_none() {
                default_index = Some(entries.len());
            }

            entries.push(ProviderEntry {
                name: cfg.name.clone(),
                provider_type,
                provider,
                models: model_list,
                default_model,
                api_key: cfg.api_key.clone(),
                endpoint: cfg.endpoint.clone(),
            });
        }

        Self { entries, default_index }
    }

    pub fn get(&self, name: &str) -> Option<&ProviderEntry> {
        self.entries.iter().find(|e| e.name == name)
    }

    pub fn default_entry(&self) -> Option<&ProviderEntry> {
        self.default_index.and_then(|i| self.entries.get(i))
    }

    pub fn default_name(&self) -> Option<&str> {
        self.default_entry().map(|e| e.name.as_str())
    }

    pub fn list(&self) -> Vec<ProviderInfo> {
        self.entries
            .iter()
            .map(|e| ProviderInfo {
                name: e.name.clone(),
                provider_type: e.provider_type.clone(),
                models: e.models.clone(),
                default_model: e.default_model.clone(),
            })
            .collect()
    }

    pub fn rebuild_from_config(&mut self, config: &AppConfig) {
        *self = Self::from_config(config);
    }
}
