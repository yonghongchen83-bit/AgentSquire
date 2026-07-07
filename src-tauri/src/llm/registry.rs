use crate::state::config::AppConfig;
use std::path::PathBuf;

pub use provider_registry::{ProviderEntry, ProviderInfo, ProviderRegistry};

/// Build a [`ProviderRegistry`] from `AppConfig` with the wire-log written to
/// the default config directory (`{config_dir}/provider-wire.log`).
pub fn from_app_config(config: &AppConfig) -> ProviderRegistry {
    let log_path = config_dir_wire_log();
    from_app_config_with_wire_log(config, log_path)
}

/// Build a [`ProviderRegistry`] from `AppConfig` using a custom wire-log path.
///
/// Pass `None` for `wire_log_path` to disable wire logging entirely, or
/// `Some(path)` to write to a specific location (used during workspace bind
/// to redirect logs under `.squire/provider-wire.log`).
pub fn from_app_config_with_wire_log(
    config: &AppConfig,
    wire_log_path: Option<PathBuf>,
) -> ProviderRegistry {
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
        wire_log_path,
    };
    ProviderRegistry::from_config(&cfg)
}

/// Returns `Some({config_dir}/provider-wire.log)` — the default wire-log path.
fn config_dir_wire_log() -> Option<PathBuf> {
    Some(crate::state::config::config_dir().join("provider-wire.log"))
}
