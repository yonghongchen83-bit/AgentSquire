use squirecli_lib::commands::conversations::sanitize_conversation_title;

#[test]
fn sanitize_conversation_title_rejects_empty() {
    let result = sanitize_conversation_title("   ".to_string());
    assert!(result.is_err());
}

#[test]
fn sanitize_conversation_title_trims_and_limits_length() {
    let title = format!("   {}   ", "x".repeat(200));
    let out = sanitize_conversation_title(title).expect("title should sanitize");
    assert_eq!(out.len(), 120);
    assert!(out.chars().all(|c| c == 'x'));
}
