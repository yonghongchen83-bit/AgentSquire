use squirecli_lib::fs::git::status;
use std::fs;

fn init_temp_repo() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(dir.path()).unwrap();
    let path = dir.path().to_str().unwrap().to_string();

    let mut cfg = repo.config().unwrap();
    cfg.set_str("user.name", "test").unwrap();
    cfg.set_str("user.email", "test@test.com").unwrap();

    let sig = git2::Signature::now("test", "test@test.com").unwrap();
    let tree_id = repo.index().unwrap().write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
        .unwrap();

    (dir, path)
}

#[test]
fn test_status_no_changes() {
    let (_dir, path) = init_temp_repo();
    let entries = status(&path).unwrap();
    let modified: Vec<_> = entries.iter().filter(|e| e.status != "added").collect();
    assert_eq!(modified.len(), 0);
}

#[test]
fn test_status_with_modification() {
    let (_dir, path) = init_temp_repo();
    fs::write(format!("{}/test.txt", path), "content").unwrap();

    let entries = status(&path).unwrap();
    let added: Vec<_> = entries.iter().filter(|e| e.status == "added").collect();
    assert_eq!(added.len(), 1);
    assert_eq!(added[0].path, "test.txt");
}
