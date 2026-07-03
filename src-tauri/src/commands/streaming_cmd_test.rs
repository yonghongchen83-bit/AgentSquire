use super::*;

#[test]
fn should_stream_live_chunks_true_for_legacy_mode() {
    assert!(should_stream_live_chunks(ContextMode::Legacy));
}

#[test]
fn should_stream_live_chunks_false_for_squire_mode() {
    assert!(!should_stream_live_chunks(ContextMode::Squire));
}
