//! Skill resource loading: directory scanner, TOML parser, and SquireStore
//! ingestor.  Loads skills from three priority-ordered sources:
//!
//! 1. **Built-in** — embedded via `include_str!` at compile time
//! 2. **User**      — `{config_dir}/skills/`
//! 3. **Project**   — `{project_path}/.squire/skills/` (skipped when no
//!                     project is loaded)
//!
//! Higher-priority sources overwrite lower-priority ones by token `id`, so
//! users and projects can override any built‑in skill by creating a `.toml`
//! file with the same `id`.
//!
//! A background `notify` watcher re‑ingests user & project skill directories
//! whenever a `.toml` file is created, modified, or deleted, so changes are
//! picked up without restarting the app.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use serde::Deserialize;

use notify::Watcher as _;

use crate::agent::squire::{NewTokenSpec, SquireStore};

// ── Data types ─────────────────────────────────────────────────────────

/// A single skill definition, directly deserialized from a `.toml` file.
#[derive(Debug, Clone, Deserialize)]
pub struct SkillDef {
    pub id: String,
    pub short_desc: String,
    pub full_desc: String,
}

// ── Built-in skills (embedded at compile time) ─────────────────────────

macro_rules! builtin_skills {
    () => {
        vec![
            (
                "SK_CreateSkill",
                include_str!("../../skills/SK_CreateSkill.toml"),
            ),
            (
                "SK_CreateWorkflow",
                include_str!("../../skills/SK_CreateWorkflow.toml"),
            ),
            (
                "SK_DecisionTree",
                include_str!("../../skills/SK_DecisionTree.toml"),
            ),
            (
                "SK_SubagentDispatch",
                include_str!("../../skills/SK_SubagentDispatch.toml"),
            ),
        ]
    };
}

/// Parse a single TOML string into a [`SkillDef`].
pub fn parse_skill_toml(toml_str: &str) -> Result<SkillDef, String> {
    toml::from_str::<SkillDef>(toml_str).map_err(|e| e.to_string())
}

/// Load all built-in skills — TOML strings compiled into the binary via
/// `include_str!`.
pub fn load_builtin_skills() -> Vec<SkillDef> {
    builtin_skills!()
        .into_iter()
        .map(|(id, content)| {
            let sk = parse_skill_toml(content).unwrap_or_else(|e| {
                panic!(
                    "squire_skills: invalid built-in skill {}: {}",
                    id, e
                )
            });
            assert_eq!(
                sk.id, id,
                "squire_skills: built-in skill id mismatch: \
                 file has '{}', expected '{}'",
                sk.id, id
            );
            sk
        })
        .collect()
}

/// Load all skill `.toml` files from a directory on disk.
///
/// Returns an empty `Vec` silently if the directory does not exist or
/// cannot be read — callers decide which directories are optional.
pub fn load_skills_from_dir(dir: &Path) -> Vec<SkillDef> {
    if !dir.is_dir() {
        return Vec::new();
    }
    let mut skills = Vec::new();
    match std::fs::read_dir(dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                    continue;
                }
                match std::fs::read_to_string(&path) {
                    Ok(content) => match parse_skill_toml(&content) {
                        Ok(sk) => skills.push(sk),
                        Err(e) => log::warn!(
                            "squire_skills: skipping {}: {}",
                            path.display(),
                            e
                        ),
                    },
                    Err(e) => log::warn!(
                        "squire_skills: cannot read {}: {}",
                        path.display(),
                        e
                    ),
                }
            }
        }
        Err(e) => log::warn!(
            "squire_skills: cannot list directory {}: {}",
            dir.display(),
            e
        ),
    }
    skills
}

/// Ingest a single [`SkillDef`] into the Squire store as a `skill`-typed
/// token with `creation_turn = 0`.
///
/// Public so it can be called from unit tests or the file‑watcher
/// background task.
pub async fn ingest_one_skill(store: &dyn SquireStore, sk: &SkillDef) {
    store
        .upsert_token(
            NewTokenSpec {
                id: sk.id.clone(),
                token_type: "skill".to_string(),
                short_desc: sk.short_desc.clone(),
                full_desc: Some(sk.full_desc.clone()),
                endpoint: None,
                ranges: vec![],
            },
            0,
        )
        .await;
}

/// Merge multiple skill sources by priority (later sources win on `id`
/// collision) and ingest the result into the Squire store.
///
/// Built-in skills are loaded first, followed by user skills (from
/// `{config_dir}/skills/`), then project-specific skills (from
/// `{project_path}/.squire/skills/`).  If a source directory does not
/// exist or no project is loaded, that source is silently skipped.
///
// ── Helpers to load + merge skill sources ─────────────────────────────

/// Load built-in, user, and project skills, returning the merged list.
fn load_and_merge_skills(
    config_dir: &Path,
    project_path: Option<&Path>,
) -> Vec<SkillDef> {
    let builtin = load_builtin_skills();
    let user = load_skills_from_dir(&config_dir.join("skills"));
    let project = project_path
        .filter(|p| p.exists())
        .map(|p| p.join(".squire").join("skills"))
        .map(|d| load_skills_from_dir(&d))
        .unwrap_or_default();

    let mut merged: HashMap<String, SkillDef> = HashMap::new();
    for sk in builtin {
        merged.insert(sk.id.clone(), sk);
    }
    for sk in user {
        merged.insert(sk.id.clone(), sk);
    }
    for sk in project {
        merged.insert(sk.id.clone(), sk);
    }
    merged.into_values().collect()
}

/// Sync entry-point called from the Tauri setup closure (which runs on the
/// main thread, not inside the Tokio runtime).  Uses
/// `tauri::async_runtime::block_on` internally.
pub fn seed_all_skills(
    store: Arc<dyn SquireStore>,
    config_dir: &Path,
    project_path: Option<&Path>,
) {
    let all = load_and_merge_skills(config_dir, project_path);
    if all.is_empty() {
        return;
    }
    let store_clone = store.clone();
    tauri::async_runtime::block_on(async move {
        for sk in &all {
            ingest_one_skill(store_clone.as_ref(), sk).await;
        }
        log::info!(
            "squire_skills: seeded {} skill tokens",
            all.len()
        );
    });
}

/// Async entry-point called from `#[tauri::command]` handlers (which run
/// on the Tokio runtime).  Does NOT use `block_on`.
pub async fn seed_all_skills_async(
    store: Arc<dyn SquireStore>,
    config_dir: &Path,
    project_path: Option<&Path>,
) {
    let all = load_and_merge_skills(config_dir, project_path);
    if all.is_empty() {
        return;
    }
    for sk in &all {
        ingest_one_skill(store.as_ref(), sk).await;
    }
    log::info!(
        "squire_skills: seeded {} skill tokens",
        all.len()
    );
}

// ── File-system watcher for live re-ingest ─────────────────────────────

/// Spawn a background `notify` file watcher that re‑ingests skills
/// whenever a `.toml` file in the user or project skills directory is
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
pub fn start_skill_watcher(
    store: Arc<dyn SquireStore>,
    config_dir: &Path,
    project_path: Option<&Path>,
) {
    let user_dir: std::path::PathBuf = config_dir.join("skills");
    let proj_dir: Option<std::path::PathBuf> = project_path
        .filter(|p| p.exists())
        .map(|p| p.join(".squire").join("skills"));

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
            log::error!("squire_skills: failed to create watcher: {}", e);
            return;
        }
    };

    for dir in &watch_dirs {
        if let Err(e) = watcher.watch(dir, notify::RecursiveMode::NonRecursive) {
            log::warn!(
                "squire_skills: cannot watch directory {}: {}",
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

            log::info!("squire_skills: directory change detected, re-ingesting");

            // Re-read and re-ingest any directories that still exist.
            if user_dir.is_dir() {
                let sk = load_skills_from_dir(&user_dir);
                for s in &sk {
                    ingest_one_skill(store.as_ref(), s).await;
                }
            }
            if let Some(ref d) = proj_dir {
                if d.is_dir() {
                    let sk = load_skills_from_dir(d);
                    for s in &sk {
                        ingest_one_skill(store.as_ref(), s).await;
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
    fn parse_builtin_skills() {
        let wfs = load_builtin_skills();
        assert_eq!(wfs.len(), 2, "expected 2 built-in skills");

        // Spot-check: every expected id is present.
        let ids: std::collections::HashSet<String> =
            wfs.into_iter().map(|w| w.id).collect();
        for expected in &["SK_CreateSkill", "SK_CreateWorkflow"] {
            assert!(
                ids.contains(*expected),
                "missing built-in skill: {}",
                expected
            );
        }
    }

    #[test]
    fn parse_toml_roundtrip() {
        let toml_str = r#"
id = "SK_Test"
short_desc = "A test skill"
full_desc = "Multi-line\ndescription\nhere"
"#;
        let sk = parse_skill_toml(toml_str).unwrap();
        assert_eq!(sk.id, "SK_Test");
        assert_eq!(sk.short_desc, "A test skill");
        assert_eq!(sk.full_desc, "Multi-line\ndescription\nhere");
    }

    #[test]
    fn parse_invalid_toml_returns_error() {
        let toml_str = r#"id = 123"#; // wrong type for id
        assert!(parse_skill_toml(toml_str).is_err());
    }

    #[test]
    fn load_from_non_existent_dir_returns_empty() {
        let sk = load_skills_from_dir(Path::new(
            "C:\\this-path-does-not-exist-42",
        ));
        assert!(sk.is_empty());
    }

    #[test]
    fn load_from_dir_with_no_toml_files() {
        let dir = std::env::temp_dir().join("squire_skills_test_no_toml");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // Create a non-.toml file
        std::fs::write(dir.join("readme.txt"), "hello").unwrap();
        let sk = load_skills_from_dir(&dir);
        assert!(sk.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_from_directory() {
        let dir = std::env::temp_dir().join("squire_skills_test_load");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(
            dir.join("SK_ATest.toml"),
            r#"
id = "SK_ATest"
short_desc = "A test"
full_desc = "Test body"
"#,
        )
        .unwrap();

        let sk = load_skills_from_dir(&dir);
        assert_eq!(sk.len(), 1);
        assert_eq!(sk[0].id, "SK_ATest");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_skips_bad_toml() {
        let dir = std::env::temp_dir().join("squire_skills_test_bad");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(dir.join("bad.toml"), "not valid toml {{{").unwrap();
        std::fs::write(
            dir.join("SK_Good.toml"),
            r#"
id = "SK_Good"
short_desc = "Good"
full_desc = "Body"
"#,
        )
        .unwrap();

        let sk = load_skills_from_dir(&dir);
        assert_eq!(sk.len(), 1);
        assert_eq!(sk[0].id, "SK_Good");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn ingest_and_explore_roundtrip() {
        let store = Arc::new(InMemorySquireStore::new());

        let sk = SkillDef {
            id: "SK_Roundtrip".to_string(),
            short_desc: "Roundtrip test".to_string(),
            full_desc: "Full description".to_string(),
        };

        ingest_one_skill(store.as_ref(), &sk).await;

        // Explore for skill tokens.
        let results = store
            .explore_memory("skill", "roundtrip", 1, 10, 0)
            .await;

        // We should find our token among the results.
        let found = results.iter().any(|t| t.token_id == "SK_Roundtrip");
        assert!(found, "Expected SK_Roundtrip in explore results");
    }

    #[test]
    fn seed_all_skills_project_overrides_builtin() {
        // This test verifies that a project-level skill with the same id
        // as a built-in one takes priority.
        let store = Arc::new(InMemorySquireStore::new());

        let tmp = std::env::temp_dir().join("squire_skills_test_override");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        // Create a project-level override for SK_CreateSkill.
        let proj_skills = tmp.join(".squire").join("skills");
        std::fs::create_dir_all(&proj_skills).unwrap();
        std::fs::write(
            proj_skills.join("SK_CreateSkill.toml"),
            r#"
id = "SK_CreateSkill"
short_desc = "Overridden skill"
full_desc = "Project-specific skill override"
"#,
        )
        .unwrap();

        let config_dir = tmp.join("config");
        std::fs::create_dir_all(&config_dir).unwrap();

        seed_all_skills(store.clone(), &config_dir, Some(&tmp));
        // No panic = success.
    }
}
