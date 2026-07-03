use super::derive_models_base_url;

#[test]
fn derive_models_base_url_strips_chat_and_message_paths() {
    assert_eq!(
        derive_models_base_url("https://api.openai.com/v1/chat/completions"),
        "https://api.openai.com/v1/chat"
    );
    assert_eq!(
        derive_models_base_url("https://api.anthropic.com/v1/messages/"),
        "https://api.anthropic.com/v1"
    );
}
