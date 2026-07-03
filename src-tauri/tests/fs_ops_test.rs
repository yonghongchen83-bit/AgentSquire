use squirecli_lib::fs::ops::{read_file, write_file, create_dir, delete_item, rename_item, list_directory, FsError};

fn unique_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("squirecli-test-{}", name));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_read_write_file() {
    let dir = unique_dir("rw");
    let path = dir.join("hello.txt");
    let path_str = path.to_str().unwrap().to_string();

    write_file(&path_str, "Hello, World!").unwrap();
    assert!(path.exists());

    let content = read_file(&path_str).unwrap();
    assert_eq!(content, "Hello, World!");
}

#[test]
fn test_read_nonexistent() {
    let err = read_file("/nonexistent/file.txt");
    assert!(matches!(err, Err(FsError::NotFound(_))));
}

#[test]
fn test_create_delete_dir() {
    let dir = unique_dir("mkdir");
    let sub = dir.join("subdir");
    let sub_str = sub.to_str().unwrap().to_string();

    create_dir(&sub_str).unwrap();
    assert!(sub.exists());
    assert!(sub.is_dir());

    delete_item(&sub_str).unwrap();
    assert!(!sub.exists());
}

#[test]
fn test_rename_item() {
    let dir = unique_dir("rename");
    let old = dir.join("old.txt");
    let new = dir.join("new.txt");
    let old_str = old.to_str().unwrap().to_string();
    let new_str = new.to_str().unwrap().to_string();

    write_file(&old_str, "rename me").unwrap();
    rename_item(&old_str, &new_str).unwrap();
    assert!(!old.exists());
    assert!(new.exists());
    assert_eq!(read_file(&new_str).unwrap(), "rename me");
}

#[test]
fn test_list_directory() {
    let dir = unique_dir("ls");
    let a = dir.join("a.txt");
    let b = dir.join("b.txt");
    let sub = dir.join("sub");
    std::fs::write(&a, "a").unwrap();
    std::fs::write(&b, "b").unwrap();
    std::fs::create_dir(&sub).unwrap();

    let entries = list_directory(dir.to_str().unwrap()).unwrap();
    assert_eq!(entries.len(), 3);
    assert!(entries.iter().any(|e| e.is_dir && e.name == "sub"));
}

#[test]
fn test_delete_nonexistent() {
    let err = delete_item("/nonexistent/path");
    assert!(matches!(err, Err(FsError::NotFound(_))));
}

#[test]
fn test_list_nonexistent() {
    let err = list_directory("/nonexistent");
    assert!(matches!(err, Err(FsError::NotFound(_))));
}
