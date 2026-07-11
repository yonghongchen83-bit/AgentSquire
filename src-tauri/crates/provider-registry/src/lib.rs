use provider_anthropic::AnthropicProvider;
use provider_core::{LlmProvider, ModelInstance, ModelOptions};
use provider_openai::OpenAIProvider;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize)]
pub struct ProviderInfo {
    pub name: String,
    pub provider_type: String,
    pub models: Vec<String>,
    pub default_model: String,
}

#[derive(Debug, Clone)]
pub struct ProviderSpec {
    pub provider_type: String,
    pub name: String,
    pub api_key: String,
    pub model: String,
    pub models: Vec<String>,
    pub endpoint: Option<String>,
    pub metadata: HashMap<String, String>,
    pub category: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProviderRegistryConfig {
    pub providers: Vec<ProviderSpec>,
    pub verbose_logging: bool,
    pub wire_log_path: Option<PathBuf>,
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

#[derive(Clone)]
pub struct ProviderRegistry {
    entries: Vec<ProviderEntry>,
    default_index: Option<usize>,
}

impl ProviderRegistry {
    pub fn from_config(config: &ProviderRegistryConfig) -> Self {
        let mut entries = Vec::new();
        let mut default_index = None;

        for cfg in &config.providers {
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
                "openai" | "openrouter" => {
                    let mut p = OpenAIProvider::new(
                        cfg.api_key.clone(),
                        default_model.clone(),
                        cfg.endpoint.clone(),
                    );
                    if let Some(path) = &config.wire_log_path {
                        p = p.with_wire_log_path(path.clone());
                    }
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

        Self {
            entries,
            default_index,
        }
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

    /// Resolve a `ModelInstance` from a provider name and model ID with
    /// optional overrides. Looks up the provider entry, merges its stored
    /// config (endpoint, api_key) with the given overrides, and returns a
    /// complete `ModelInstance`.
    pub fn resolve_model_instance(
        &self,
        provider_name: &str,
        model: &str,
        options: Option<ModelOptions>,
    ) -> Result<ModelInstance, String> {
        let entry = self
            .get(provider_name)
            .ok_or_else(|| format!("Provider '{}' not found in registry", provider_name))?;
        Ok(ModelInstance {
            provider_name: provider_name.to_string(),
            model: model.to_string(),
            endpoint: entry.endpoint.clone(),
            api_key: if entry.api_key.is_empty() {
                None
            } else {
                Some(entry.api_key.clone())
            },
            options: options.unwrap_or_default(),
        })
    }

    /// Resolve a provider `Arc` for a given `ModelInstance`. Returns the
    /// provider entry's `Arc<dyn LlmProvider>` and the resolved model string.
    ///
    /// This is the primary way the engine should obtain a provider — it keeps
    /// the engine loop decoupled from registry internals.
    pub fn resolve_provider_for_instance(
        &self,
        instance: &ModelInstance,
    ) -> Result<(Arc<dyn LlmProvider>, String), String> {
        let entry = self
            .get(&instance.provider_name)
            .ok_or_else(|| format!("Provider '{}' not found", instance.provider_name))?;
        let model = if instance.model.is_empty() {
            entry.default_model.clone()
        } else {
            instance.model.clone()
        };
        Ok((entry.provider.clone(), model))
    }

    /// Convenience: resolve both provider+model and produce the effective
    /// model string (honouring the instance's model field, falling back to
    /// the entry's default).
    pub fn resolve_model(&self, instance: &ModelInstance) -> String {
        if !instance.model.is_empty() {
            return instance.model.clone();
        }
        self.get(&instance.provider_name)
            .map(|e| e.default_model.clone())
            .unwrap_or_default()
    }

    pub fn rebuild_from_config(&mut self, config: &ProviderRegistryConfig) {
        *self = Self::from_config(config);
    }
}
