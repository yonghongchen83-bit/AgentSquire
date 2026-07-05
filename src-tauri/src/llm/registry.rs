use crate::state::config::AppConfig;

pub use provider_registry::{ProviderEntry, ProviderInfo, ProviderRegistry};

pub fn from_app_config(config: &AppConfig) -> ProviderRegistry {
    let cfg = provider_registry::ProviderRegistryConfig {
        providers: config
            .llm_providers
            .iter()
            .map(|p| provider_registry::ProviderSpec {
                provider_type: p.provider_type.clone(),
                name: p.name.clone(),
                api_key: p.api_key.clone(),
                model: p.model.clone(),
                models: p.models.clone(),
                endpoint: p.endpoint.clone(),
                metadata: p.metadata.clone(),
                category: p.category.clone(),
            })
            .collect(),
        verbose_logging: config.verbose_logging,
        wire_log_path: Some(crate::state::config::config_dir().join("provider-wire.log")),
    };
    ProviderRegistry::from_config(&cfg)
}
