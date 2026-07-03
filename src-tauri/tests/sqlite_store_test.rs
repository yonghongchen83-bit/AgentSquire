use squirecli_lib::state::db::Database;
use squirecli_lib::storage::conversation_store::{ContextMode, ConversationStore, NewSession};

fn temp_db() -> (Database, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(&dir.path().join("test.db")).unwrap();
    (db, dir)
}

#[tokio::test]
async fn list_sessions_reports_each_sessions_context_mode() {
    let (db, _dir) = temp_db();
    let legacy = db
        .create_session(NewSession {
            title: "Legacy one".into(),
            context_mode: None,
        })
        .await
        .unwrap();
    let squire = db
        .create_session(NewSession {
            title: "Squire one".into(),
            context_mode: Some(ContextMode::Squire),
        })
        .await
        .unwrap();

    let summaries = db.list_sessions().await.unwrap();
    let legacy_summary = summaries.iter().find(|s| s.id == legacy.id).unwrap();
    let squire_summary = summaries.iter().find(|s| s.id == squire.id).unwrap();
    assert_eq!(legacy_summary.context_mode, ContextMode::Legacy);
    assert_eq!(squire_summary.context_mode, ContextMode::Squire);
}

#[tokio::test]
async fn list_sessions_defaults_to_legacy_when_context_mode_omitted() {
    let (db, _dir) = temp_db();
    let created = db
        .create_session(NewSession {
            title: "Default mode".into(),
            context_mode: None,
        })
        .await
        .unwrap();
    assert_eq!(created.context_mode, ContextMode::Legacy);

    let summaries = db.list_sessions().await.unwrap();
    let summary = summaries.iter().find(|s| s.id == created.id).unwrap();
    assert_eq!(summary.context_mode, ContextMode::Legacy);
}
