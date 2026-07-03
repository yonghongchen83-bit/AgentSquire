use squirecli_lib::commands::git::{git_status_impl, git_diff_impl, git_log_impl, git_branches_impl};

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
