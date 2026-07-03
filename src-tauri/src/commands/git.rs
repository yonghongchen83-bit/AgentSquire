pub fn git_status_impl(path: Option<String>) -> Result<Vec<crate::fs::git::GitStatus>, String> {
    let p = path.as_deref().unwrap_or(".");
    crate::fs::git::status(p).map_err(|e| e.to_string())
}

pub fn git_diff_impl(path: String, staged: bool) -> Result<Vec<crate::fs::git::GitDiff>, String> {
    crate::fs::git::diff(&path, staged).map_err(|e| e.to_string())
}

pub fn git_log_impl(path: String, max_count: i32) -> Result<Vec<crate::fs::git::GitLogEntry>, String> {
    crate::fs::git::log(&path, max_count).map_err(|e| e.to_string())
}

pub fn git_branches_impl(path: String) -> Result<Vec<crate::fs::git::GitBranch>, String> {
    crate::fs::git::branches(&path).map_err(|e| e.to_string())
}
