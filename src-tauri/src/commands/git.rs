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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_status_impl_errors_for_non_repo() {
        let dir = tempfile::tempdir().expect("tempdir should be created");
        let result = git_status_impl(Some(dir.path().to_string_lossy().to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn git_wrappers_are_callable() {
        let dir = tempfile::tempdir().expect("tempdir should be created");
        let path = dir.path().to_string_lossy().to_string();

        // Ensure wrappers execute and return a Result without panicking.
        let _ = git_diff_impl(path.clone(), false);
        let _ = git_log_impl(path.clone(), 10);
        let _ = git_branches_impl(path);
    }
}
