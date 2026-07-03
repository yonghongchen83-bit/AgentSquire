use squirecli_lib::commands::watcher_cmd::{watch_started_event, watch_directory_impl};
use squirecli_lib::fs::watcher::FileWatcher;
use std::sync::Mutex;

#[test]
fn watch_started_event_shape_is_stable() {
    let evt = watch_started_event("/tmp/project".to_string());
    assert_eq!(evt.kind, "watch-started");
    assert_eq!(evt.paths, vec!["/tmp/project".to_string()]);
}

#[test]
fn watch_directory_impl_accepts_existing_directory() {
    let (watcher, _rx) = FileWatcher::new();
    let watcher = Mutex::new(watcher);
    let dir = tempfile::tempdir().expect("tempdir should be created");

    let result = watch_directory_impl(&watcher, dir.path().to_string_lossy().as_ref());
    assert!(result.is_ok());
}
