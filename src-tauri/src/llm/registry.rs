use std::collections::HashMap;

use super::openai::OpenAIProvider;
use super::anthropic::AnthropicProvider;
use super::provider::LlmProvider;
use crate::state::config::LlmConfig;

pub struct ProviderRegistry {
    providers: HashMap<String, Box<dyn LlmProvider>>,
    default: Option<String>,
}

impl ProviderRegistry {
    pub fn from_config(config: &LlmConfig) -> Self {
        let mut providers: HashMap<String, Box<dyn LlmProvider>> = HashMap::new();
        let mut default: Option<String> = None;

        for cfg in &config.providers {
            let name = cfg.name.clone();
            match cfg.provider_type.to_lowercase().as_str() {
                "openai" => {
                    let provider = OpenAIProvider::new(
                        cfg.api_key.clone(),
                        cfg.model.clone(),
                        cfg.base_url.clone(),
                    );
                    providers.insert(name.clone(), Box::new(provider));
                }
                "anthropic" => {
                    let provider = AnthropicProvider::new(
                        cfg.api_key.clone(),
                        cfg.model.clone(),
                        cfg.base_url.clone(),
                    );
                    providers.insert(name.clone(), Box::new(provider));
                }
                _ => {
                    tracing::warn!("Unknown LLM provider type: {}", cfg.provider_type);
                    continue;
                }
            }
            if default.is_none() {
                default = Some(name);
            }
        }

        Self { providers, default }
    }

    pub fn get(&self, name: &str) -> Option<&dyn LlmProvider> {
        self.providers.get(name).map(|p| p.as_ref())
    }

    pub fn default(&self) -> Option<&dyn LlmProvider> {
        self.default
            .as_ref()
            .and_then(|name| self.get(name))
    }

    pub fn default_name(&self) -> Option<&str> {
        self.default.as_deref()
    }

    pub fn list(&self) -> Vec<(String, String)> {
        self.providers
            .iter()
            .map(|(name, p)| (name.clone(), p.provider_name().to_string()))
            .collect()
    }
}
