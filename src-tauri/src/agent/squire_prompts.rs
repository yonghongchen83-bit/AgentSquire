//! External system prompt loading with three-tier override:
//!   1. Built-in (embedded via include_str!)
//!   2. User   ({config_dir}/prompts/system-prompt.md)
//!   3. Project ({project_path}/.squire/prompts/system-prompt.md)
//!
//! Later sources override earlier ones: project > user > built-in.
//! Individual files override independently — you can override just the
//! system prompt without touching the others.
//!
//! If seed_all_prompts has not been called (e.g. during early startup or
//! in unit tests), get_prompt falls back to the built-in content so that
//! code does not panic.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

// ── Prompt file entries ──────────────────────────────────────────────

/// All editable prompt files. Add new entries here when introducing a new
/// prompt that you want to make externally overridable.
static PROMPT_FILES: &[PromptFile] = &[PromptFile {
    name: "system-prompt.md",
    builtin: include_str!("../../prompts/system-prompt.md"),
}];

struct PromptFile {
    /// Filename used in all three tiers (e.g. "system-prompt.md").
    name: &'static str,
    /// Built-in content embedded at compile time.
    builtin: &'static str,
}

// ── Merged store ─────────────────────────────────────────────────────

static MERGED: Mutex<Option<HashMap<&'static str, String>>> = Mutex::new(None);

/// (Re)load all prompts from all three tiers and merge them.
/// Call once at startup from `setup_cmd.rs`, and again whenever you want
/// to hot-reload (e.g. on file watcher event).
pub fn seed_all_prompts(config_dir: &Path, project_path: Option<&Path>) {
    let mut merged: HashMap<&'static str, String> = HashMap::new();

    for pf in PROMPT_FILES {
        // 1. Built-in (lowest priority)
        let mut content = pf.builtin.to_string();

        // 2. User override at {config_dir}/prompts/{name}
        let user_path = config_dir.join("prompts").join(pf.name);
        if let Ok(disk) = std::fs::read_to_string(&user_path) {
            content = disk;
        }

        // 3. Project override at {project_path}/.squire/prompts/{name}
        if let Some(proj) = project_path {
            let proj_path = proj.join(".squire").join("prompts").join(pf.name);
            if let Ok(disk) = std::fs::read_to_string(&proj_path) {
                content = disk;
            }
        }

        merged.insert(pf.name, content);
    }

    *MERGED.lock().unwrap() = Some(merged);
}

/// Retrieve a merged prompt by its filename.
/// Falls back to the built-in content if seeding has not happened yet,
/// so code (including tests) can work without calling seed_all_prompts.
pub fn get_prompt(name: &str) -> String {
    let guard = MERGED.lock().unwrap();
    if let Some(map) = guard.as_ref() {
        if let Some(content) = map.get(name) {
            return content.clone();
        }
    }
    // Fallback: find the matching built-in entry
    PROMPT_FILES
        .iter()
        .find(|pf| pf.name == name)
        .map(|pf| pf.builtin.to_string())
        .unwrap_or_default()
}

/// Convenience accessor for the main system prompt.
pub fn system_prompt() -> String {
    get_prompt("system-prompt.md")
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_system_prompt_is_not_empty() {
        // Before seed_all_prompts is called, get_prompt falls back to
        // the built-in content.
        let prompt = get_prompt("system-prompt.md");
        assert!(!prompt.is_empty());
        assert!(prompt.contains("Context Squire"));
    }

    #[test]
    fn seed_and_retrieve() {
        let dir = std::env::temp_dir().join("squire_prompts_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        seed_all_prompts(&dir, None);
        let prompt = get_prompt("system-prompt.md");
        assert!(prompt.contains("Context Squire"));
        assert!(!prompt.is_empty());
    }

    #[test]
    fn override_hierarchy_is_correct() {
        // Use distinct directories per test to avoid OnceLock races
        // when tests run in parallel.
        let dir = std::env::temp_dir().join("squire_prompts_override_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir.join("prompts")).unwrap();
        // User override: write a unique marker
        std::fs::write(
            dir.join("prompts").join("system-prompt.md"),
            "USER OVERRIDE",
        )
        .unwrap();

        // Re-seed with NO project path — should get user override
        let _ = std::fs::remove_dir_all(&dir.join("..").join("squire_prompts_empty_test"));
        seed_all_prompts(&dir, None);
        assert_eq!(get_prompt("system-prompt.md"), "USER OVERRIDE");
    }

    #[test]
    fn project_override_wins_over_user() {
        let dir = std::env::temp_dir().join("squire_prompts_project_wins");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir.join("prompts")).unwrap();
        std::fs::write(
            dir.join("prompts").join("system-prompt.md"),
            "USER PROMPT",
        )
        .unwrap();
        let proj_dir = std::env::temp_dir().join("squire_prompts_project_wins_proj");
        let _ = std::fs::remove_dir_all(&proj_dir);
        std::fs::create_dir_all(&proj_dir.join(".squire").join("prompts")).unwrap();
        std::fs::write(
            proj_dir
                .join(".squire")
                .join("prompts")
                .join("system-prompt.md"),
            "PROJECT PROMPT",
        )
        .unwrap();

        seed_all_prompts(&dir, Some(&proj_dir));
        assert_eq!(get_prompt("system-prompt.md"), "PROJECT PROMPT");
    }
}
