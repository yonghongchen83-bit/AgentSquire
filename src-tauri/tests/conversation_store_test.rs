use squirecli_lib::storage::conversation_store::{MessageRole, ContextMode, NewSession};

#[test]
fn test_message_role_roundtrip() {
    assert_eq!(MessageRole::User.as_str(), "user");
    assert_eq!(MessageRole::Assistant.as_str(), "assistant");
    assert_eq!(MessageRole::System.as_str(), "system");
    assert_eq!(
        MessageRole::from_str("user").unwrap() as usize,
        MessageRole::User as usize
    );
    assert!(MessageRole::from_str("unknown").is_none());
    assert_eq!(
        serde_json::to_string(&MessageRole::User).unwrap(),
        "\"user\""
    );
    assert_eq!(
        serde_json::to_string(&MessageRole::Assistant).unwrap(),
        "\"assistant\""
    );
}

#[test]
fn test_session_creation() {
    let new = NewSession {
        title: "Test Session".into(),
        context_mode: None,
    };
    assert_eq!(new.title, "Test Session");
}

#[test]
fn test_context_mode_defaults_to_legacy_and_roundtrips() {
    assert_eq!(ContextMode::default(), ContextMode::Legacy);
    assert_eq!(ContextMode::Legacy.as_str(), "legacy");
    assert_eq!(ContextMode::Squire.as_str(), "squire");
    assert_eq!(ContextMode::from_str("legacy"), Some(ContextMode::Legacy));
    assert_eq!(ContextMode::from_str("squire"), Some(ContextMode::Squire));
    assert!(ContextMode::from_str("unknown").is_none());
}
