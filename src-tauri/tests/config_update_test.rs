use squirecli_lib::commands::config_update::{check_update_impl, load_config_impl};

#[test]
fn check_update_impl_shape_is_stable() {
    let payload = check_update_impl();
    assert_eq!(payload["available"], serde_json::json!(false));
    assert!(payload.get("version").is_some());
    assert!(payload.get("body").is_some());
}

#[test]
fn load_config_impl_returns_valid_config() {
    let cfg = load_config_impl().expect("load_config_impl should succeed");
    assert!(!cfg.theme.is_empty());
}
