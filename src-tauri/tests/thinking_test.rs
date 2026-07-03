use squirecli_lib::llm::thinking::normalize_level;

#[test]
fn defaults_to_mid_when_missing() {
    assert_eq!(normalize_level(None), "mid");
}

#[test]
fn accepts_supported_levels() {
    assert_eq!(normalize_level(Some("none".to_string())), "none");
    assert_eq!(normalize_level(Some("low".to_string())), "low");
    assert_eq!(normalize_level(Some("mid".to_string())), "mid");
    assert_eq!(normalize_level(Some("high".to_string())), "high");
}

#[test]
fn normalizes_case_and_rejects_unknown() {
    assert_eq!(normalize_level(Some("HIGH".to_string())), "high");
    assert_eq!(normalize_level(Some("unsupported".to_string())), "mid");
}
