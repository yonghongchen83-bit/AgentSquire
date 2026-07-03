use crate::state::config::{self, AppConfig};

pub fn load_config_impl() -> Result<AppConfig, String> {
    config::load_config().map_err(|e| e.to_string())
}

pub fn check_update_impl() -> serde_json::Value {
    serde_json::json!({
        "available": false,
        "version": null,
        "body": null,
    })
}
