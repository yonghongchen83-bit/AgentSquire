use super::*;

#[test]
fn should_stream_live_chunks_true_for_all_modes() {
    assert!(should_stream_live_chunks(ContextMode::Legacy));
    assert!(should_stream_live_chunks(ContextMode::Squire));
}
