//! External system prompt loading with four-tier override:
//!   1. Built-in  (loaded from `<builtin_root>/prompts/` at runtime)
//!   2. Embedded  (via `include_str!`, fallback if disk read fails)
//!   3. User      ({config_dir}/prompts/system-prompt.md)
//!   4. Project   ({project_path}/.squire/prompts/system-prompt.md)
//!
//! Later sources override earlier ones: project > user > embedded > builtin-disk.
//! Individual files override independently — you can override just the
//! system prompt without touching the others.
//!
//! The built-in file is read from disk at runtime (not at compile time),
//! so editing `prompts/system-prompt.md` does **not** trigger a Rust
//! recompilation.  The `include_str!` fallback is only used when the disk
//! read fails (e.g. running the binary outside the source tree).

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

// ── Prompt file entries ──────────────────────────────────────────────

/// All editable prompt files. Add new entries here when introducing a new
/// prompt that you want to make externally overridable.
static PROMPT_FILES: &[PromptFile] = &[
    PromptFile {
        name: "system-prompt.md",
        // Built-in content embedded at compile time as a fallback.
        // This is used when the disk-based builtin file cannot be read
        // (e.g. running a release binary outside the source tree).
        builtin: include_str!("../../prompts/system-prompt.md"),
    },
    PromptFile {
        name: "system-prompt-phase2.md",
        builtin: include_str!("../../prompts/system-prompt-phase2.md"),
    },
    PromptFile {
        name: "system-prompt-formatter.md",
        builtin: include_str!("../../prompts/system-prompt-formatter.md"),
    },
];

struct PromptFile {
    /// Filename used in all three tiers (e.g. "system-prompt.md").
    name: &'static str,
    /// Built-in content embedded at compile time (emergency fallback).
    builtin: &'static str,
}

// ── Merged store ─────────────────────────────────────────────────────

static MERGED: Mutex<Option<HashMap<&'static str, String>>> = Mutex::new(None);

/// (Re)load all prompts from all four tiers and merge them.
///
/// `builtin_dir` is the source-tree root where `prompts/<name>` lives
/// (e.g. the Tauri project root containing `src-tauri/prompts/`).  Pass
/// `std::env::current_dir()` or the project root; during development this
/// should be the repo root.
///
/// Call once at startup from `setup_cmd.rs`, and again whenever you want
/// to hot-reload (e.g. on file watcher event).
pub fn seed_all_prompts(
    builtin_dir: Option<&Path>,
    config_dir: &Path,
    project_path: Option<&Path>,
) {
    let mut merged: HashMap<&'static str, String> = HashMap::new();

    for pf in PROMPT_FILES {
        // 1. Built-in from disk (lowest priority — avoids recompilation).
        //    Falls back to the compile-time `include_str!` content.
        let mut content = if let Some(root) = builtin_dir {
            let builtin_path = root.join("src-tauri").join("prompts").join(pf.name);
            std::fs::read_to_string(&builtin_path).unwrap_or_else(|_| pf.builtin.to_string())
        } else {
            pf.builtin.to_string()
        };

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
///
/// Use `"system-prompt.md"` for Phase 1 (explore + respond) and
/// `"system-prompt-phase2.md"` for Phase 2 (token generation only).
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

/// Convenience accessor for the main system prompt (Phase 1).
pub fn system_prompt() -> String {
    get_prompt("system-prompt.md")
}

/// Convenience accessor for the Phase 2 token-generation prompt.
pub fn system_prompt_phase2() -> String {
    get_prompt("system-prompt-phase2.md")
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

        seed_all_prompts(None, &dir, None);
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
        seed_all_prompts(None, &dir, None);
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

        seed_all_prompts(None, &dir, Some(&proj_dir));
        assert_eq!(get_prompt("system-prompt.md"), "PROJECT PROMPT");
    }
}
