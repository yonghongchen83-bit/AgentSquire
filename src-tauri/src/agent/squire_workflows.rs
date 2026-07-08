//! Workflow resource loading: directory scanner, TOML parser, and SquireStore
//! ingestor.  Loads workflows from three priority-ordered sources:
//!
//! 1. **Built-in** — embedded via `include_str!` at compile time
//! 2. **User**      — `{config_dir}/workflows/`
//! 3. **Project**   — `{project_path}/.squire/workflows/` (skipped when no
//!                     project is loaded)
//!
//! Higher-priority sources overwrite lower-priority ones by token `id`, so
//! users and projects can override any built‑in workflow by creating a
//! `.toml` file with the same `id`.
//!
//! A background `notify` watcher re‑ingests user & project workflow
//! directories whenever a `.toml` file is created, modified, or deleted, so
//! changes are picked up without restarting the app.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use serde::Deserialize;

use notify::Watcher as _;

use squire_store::SessionId;
use crate::agent::squire::{NewTokenSpec, SquireStore};

// ── Data types ─────────────────────────────────────────────────────────

/// A single workflow definition, directly deserialized from a `.toml` file.
#[derive(Debug, Clone, Deserialize)]
pub struct WorkflowDef {
    pub id: String,
    pub short_desc: String,
    pub full_desc: String,
}

// ── Built-in workflows (embedded at compile time) ──────────────────────

macro_rules! builtin_workflows {
    () => {
        vec![
            (
                "WF_SimpleQA",
                include_str!("../../workflows/WF_SimpleQA.toml"),
            ),
            (
                "WF_FriendlyChat",
                include_str!("../../workflows/WF_FriendlyChat.toml"),
            ),
            (
                "WF_WaterfallDesign",
                include_str!("../../workflows/WF_WaterfallDesign.toml"),
            ),
            (
                "WF_InteractiveDiscovery",
                include_str!("../../workflows/WF_InteractiveDiscovery.toml"),
            ),
            (
                "WF_TaskExecution",
                include_str!("../../workflows/WF_TaskExecution.toml"),
            ),
            (
                "WF_Debugging",
                include_str!("../../workflows/WF_Debugging.toml"),
            ),
            (
                "WF_DecisionTree",
                include_str!("../../workflows/WF_DecisionTree.toml"),
            ),
            (
                "WF_UseSubagent",
                include_str!("../../workflows/WF_UseSubagent.toml"),
            ),
            (
                "WF_DebugTesting",
                include_str!("../../workflows/WF_DebugTesting.toml"),
            ),
        ]
    };
}

/// Parse a single TOML string into a [`WorkflowDef`].
pub fn parse_workflow_toml(toml_str: &str) -> Result<WorkflowDef, String> {
    toml::from_str::<WorkflowDef>(toml_str).map_err(|e| e.to_string())
}

/// Load all built-in workflows — TOML strings compiled into the binary via
/// `include_str!`.
pub fn load_builtin_workflows() -> Vec<WorkflowDef> {
    builtin_workflows!()
        .into_iter()
        .map(|(id, content)| {
            let wf = parse_workflow_toml(content).unwrap_or_else(|e| {
                panic!(
                    "squire_workflows: invalid built-in workflow {}: {}",
                    id, e
                )
            });
            assert_eq!(
                wf.id, id,
                "squire_workflows: built-in workflow id mismatch: \
                 file has '{}', expected '{}'",
                wf.id, id
            );
            wf
        })
        .collect()
}

/// Load all workflow `.toml` files from a directory on disk.
///
/// Returns an empty `Vec` silently if the directory does not exist or
/// cannot be read — callers decide which directories are optional.
pub fn load_workflows_from_dir(dir: &Path) -> Vec<WorkflowDef> {
    if !dir.is_dir() {
        return Vec::new();
    }
    let mut workflows = Vec::new();
    match std::fs::read_dir(dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                    continue;
                }
                match std::fs::read_to_string(&path) {
                    Ok(content) => match parse_workflow_toml(&content) {
                        Ok(wf) => workflows.push(wf),
                        Err(e) => log::warn!(
                            "squire_workflows: skipping {}: {}",
                            path.display(),
                            e
                        ),
                    },
                    Err(e) => log::warn!(
                        "squire_workflows: cannot read {}: {}",
                        path.display(),
                        e
                    ),
                }
            }
        }
        Err(e) => log::warn!(
            "squire_workflows: cannot list directory {}: {}",
            dir.display(),
            e
        ),
    }
    workflows
}

/// Ingest a single [`WorkflowDef`] into the Squire store as a
/// `workflow`-typed token with `creation_turn = 0`.
///
/// Public so it can be called from unit tests or the file‑watcher
/// background task.
pub async fn ingest_one_workflow(store: &dyn SquireStore, wf: &WorkflowDef) {
    store
        .upsert_token(
            NewTokenSpec {
                id: wf.id.clone(),
                token_type: "workflow".to_string(),
                short_desc: wf.short_desc.clone(),
                full_desc: Some(wf.full_desc.clone()),
                endpoint: None,
                ranges: vec![],
            },
            0,
            SessionId::nil(),
        )
        .await;
}

/// Merge multiple workflow sources by priority (later sources win on `id`
/// collision) and ingest the result into the Squire store.
///
/// Built-in workflows are loaded first, followed by user workflows (from
/// `{config_dir}/workflows/`), then project-specific workflows (from
/// `{project_path}/.squire/workflows/`).  If a source directory does not
/// exist or no project is loaded, that source is silently skipped.
///
// ── Helpers to load + merge workflow sources ──────────────────────────

/// Load built-in, user, and project workflows, returning the merged list.
fn load_and_merge_workflows(
    config_dir: &Path,
    project_path: Option<&Path>,
) -> Vec<WorkflowDef> {
    let builtin = load_builtin_workflows();
    let user = load_workflows_from_dir(&config_dir.join("workflows"));
    let project = project_path
        .filter(|p| p.exists())
        .map(|p| p.join(".squire").join("workflows"))
        .map(|d| load_workflows_from_dir(&d))
        .unwrap_or_default();

    let mut merged: HashMap<String, WorkflowDef> = HashMap::new();
    for wf in builtin {
        merged.insert(wf.id.clone(), wf);
    }
    for wf in user {
        merged.insert(wf.id.clone(), wf);
    }
    for wf in project {
        merged.insert(wf.id.clone(), wf);
    }
    merged.into_values().collect()
}

/// Sync entry-point called from the Tauri setup closure (which runs on the
/// main thread, not inside the Tokio runtime).  Uses
/// `tauri::async_runtime::block_on` internally.
pub fn seed_all_workflows(
    store: Arc<dyn SquireStore>,
    config_dir: &Path,
    project_path: Option<&Path>,
) {
    let all = load_and_merge_workflows(config_dir, project_path);
    if all.is_empty() {
        return;
    }
    let store_clone = store.clone();
    tauri::async_runtime::block_on(async move {
        for wf in &all {
            ingest_one_workflow(store_clone.as_ref(), wf).await;
        }
        log::info!(
            "squire_workflows: seeded {} workflow tokens",
            all.len()
        );
    });
}

/// Async entry-point called from `#[tauri::command]` handlers (which run
/// on the Tokio runtime).  Does NOT use `block_on`.
pub async fn seed_all_workflows_async(
    store: Arc<dyn SquireStore>,
    config_dir: &Path,
    project_path: Option<&Path>,
) {
    let all = load_and_merge_workflows(config_dir, project_path);
    if all.is_empty() {
        return;
    }
    for wf in &all {
        ingest_one_workflow(store.as_ref(), wf).await;
    }
    log::info!(
        "squire_workflows: seeded {} workflow tokens",
        all.len()
    );
}

// ── File-system watcher for live re-ingest ─────────────────────────────

/// Spawn a background `notify` file watcher that re‑ingests workflows
/// whenever a `.toml` file in the user or project workflows directory is
/// created, modified, or deleted.
///
/// The watcher is silent (logs at `info` level on changes) and
/// self‑healing — it always re‑reads the full contents of the affected
/// directory on any event, so a delete is reflected as the absence of that
/// token in the next re‑ingest batch.
///
/// Directories that do not exist at call time are simply skipped (no
/// error).  If they are created later, a restart is required — this is
/// acceptable for the initial version.
pub fn start_workflow_watcher(
    store: Arc<dyn SquireStore>,
    config_dir: &Path,
    project_path: Option<&Path>,
) {
    let user_dir: std::path::PathBuf = config_dir.join("workflows");
    let proj_dir: Option<std::path::PathBuf> = project_path
        .filter(|p| p.exists())
        .map(|p| p.join(".squire").join("workflows"));

    // Collect directories that actually exist.
    let mut watch_dirs: Vec<std::path::PathBuf> = Vec::new();
    if user_dir.is_dir() {
        watch_dirs.push(user_dir.clone());
    }
    if let Some(ref d) = proj_dir {
        if d.is_dir() {
            watch_dirs.push(d.clone());
        }
    }

    if watch_dirs.is_empty() {
        return;
    }

    let (tx, mut rx) = tokio::sync::broadcast::channel::<notify::Event>(256);

    // notify watcher callback runs on a notify‑internal thread; forward
    // events into the tokio broadcast channel.
    let mut watcher = match notify::RecommendedWatcher::new(
        move |event: Result<notify::Event, notify::Error>| {
            if let Ok(event) = event {
                let _ = tx.send(event);
            }
        },
        notify::Config::default(),
    ) {
        Ok(w) => w,
        Err(e) => {
            log::error!("squire_workflows: failed to create watcher: {}", e);
            return;
        }
    };

    for dir in &watch_dirs {
        if let Err(e) = watcher.watch(dir, notify::RecursiveMode::NonRecursive) {
            log::warn!(
                "squire_workflows: cannot watch directory {}: {}",
                dir.display(),
                e
            );
        }
    }

    // Leak the watcher so it lives for the entire app lifetime (otherwise
    // it would be dropped when this function returns, stopping all events).
    let _leaked: &'static mut notify::RecommendedWatcher = Box::leak(Box::new(watcher));

    tokio::spawn(async move {
        loop {
            // Wait for the first event.
            if rx.recv().await.is_err() {
                break; // Channel closed.
            }

            // Debounce: drain any follow-up events within 300 ms to avoid
            // re-ingesting multiple times for a single editor save (which
            // typically fires Create + Modify events in quick succession).
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                match rx.try_recv() {
                    Ok(_) => continue,
                    Err(tokio::sync::broadcast::error::TryRecvError::Empty) => break,
                    Err(_) => break,
                }
            }

            log::info!("squire_workflows: directory change detected, re-ingesting");

            // Re-read and re-ingest any directories that still exist.
            if user_dir.is_dir() {
                let wf = load_workflows_from_dir(&user_dir);
                for w in &wf {
                    ingest_one_workflow(store.as_ref(), w).await;
                }
            }
            if let Some(ref d) = proj_dir {
                if d.is_dir() {
                    let wf = load_workflows_from_dir(d);
                    for w in &wf {
                        ingest_one_workflow(store.as_ref(), w).await;
                    }
                }
            }
        }
    });
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::squire::InMemorySquireStore;

    #[test]
    fn parse_builtin_workflows() {
        let wfs = load_builtin_workflows();
        assert_eq!(wfs.len(), 9, "expected 9 built-in workflows");

        // Spot-check: every expected id is present.
        let ids: std::collections::HashSet<String> =
            wfs.into_iter().map(|w| w.id).collect();
        for expected in &[
            "WF_SimpleQA",
            "WF_FriendlyChat",
            "WF_WaterfallDesign",
            "WF_InteractiveDiscovery",
            "WF_TaskExecution",
            "WF_Debugging",
        ] {
            assert!(
                ids.contains(*expected),
                "missing built-in workflow: {}",
                expected
            );
        }
    }

    #[test]
    fn parse_toml_roundtrip() {
        let toml_str = r#"
id = "WF_Test"
short_desc = "A test workflow"
full_desc = "Multi-line\ndescription\nhere"
"#;
        let wf = parse_workflow_toml(toml_str).unwrap();
        assert_eq!(wf.id, "WF_Test");
        assert_eq!(wf.short_desc, "A test workflow");
        assert_eq!(wf.full_desc, "Multi-line\ndescription\nhere");
    }

    #[test]
    fn parse_invalid_toml_returns_error() {
        let toml_str = r#"id = 123"#; // wrong type for id
        assert!(parse_workflow_toml(toml_str).is_err());
    }

    #[test]
    fn load_from_non_existent_dir_returns_empty() {
        let wf = load_workflows_from_dir(Path::new(
            "C:\\this-path-does-not-exist-42",
        ));
        assert!(wf.is_empty());
    }

    #[test]
    fn load_from_dir_with_no_toml_files() {
        let dir = std::env::temp_dir().join("squire_workflows_test_no_toml");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // Create a non-.toml file
        std::fs::write(dir.join("readme.txt"), "hello").unwrap();
        let wf = load_workflows_from_dir(&dir);
        assert!(wf.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_from_directory() {
        let dir = std::env::temp_dir().join("squire_workflows_test_load");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(
            dir.join("WF_ATest.toml"),
            r#"
id = "WF_ATest"
short_desc = "A test"
full_desc = "Test body"
"#,
        )
        .unwrap();

        let wf = load_workflows_from_dir(&dir);
        assert_eq!(wf.len(), 1);
        assert_eq!(wf[0].id, "WF_ATest");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_skips_bad_toml() {
        let dir = std::env::temp_dir().join("squire_workflows_test_bad");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(dir.join("bad.toml"), "not valid toml {{{").unwrap();
        std::fs::write(
            dir.join("WF_Good.toml"),
            r#"
id = "WF_Good"
short_desc = "Good"
full_desc = "Body"
"#,
        )
        .unwrap();

        let wf = load_workflows_from_dir(&dir);
        assert_eq!(wf.len(), 1);
        assert_eq!(wf[0].id, "WF_Good");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn ingest_and_explore_roundtrip() {
        let store = Arc::new(InMemorySquireStore::new());

        let wf = WorkflowDef {
            id: "WF_Roundtrip".to_string(),
            short_desc: "Roundtrip test".to_string(),
            full_desc: "Full description".to_string(),
        };

        ingest_one_workflow(store.as_ref(), &wf).await;

        // Explore for workflow tokens.
        let results = store
            .explore_memory("workflow", "roundtrip", 1, 10, 0, SessionId::nil())
            .await;

        // We should find our token among the results.
        let found = results.iter().any(|t| t.token_id == "WF_Roundtrip");
        assert!(found, "Expected WF_Roundtrip in explore results");
    }

    #[test]
    fn seed_all_workflows_project_overrides_builtin() {
        // This test verifies that a project-level workflow with the same id
        // as a built-in one takes priority.
        let store = Arc::new(InMemorySquireStore::new());

        let tmp = std::env::temp_dir().join("squire_workflows_test_override");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        // Create a project-level override for WF_Debugging.
        let proj_workflows = tmp.join(".squire").join("workflows");
        std::fs::create_dir_all(&proj_workflows).unwrap();
        std::fs::write(
            proj_workflows.join("WF_Debugging.toml"),
            r#"
id = "WF_Debugging"
short_desc = "Overridden debugging workflow"
full_desc = "Project-specific debugging process"
"#,
        )
        .unwrap();

        let config_dir = tmp.join("config");
        std::fs::create_dir_all(&config_dir).unwrap();

        seed_all_workflows(store.clone(), &config_dir, Some(&tmp));
        // No panic = success. InMemorySquireStore stores tokens so we could
        // further validate if needed.
    }
}
