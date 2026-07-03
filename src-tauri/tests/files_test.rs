use squirecli_lib::commands::files::{
    write_file_impl, read_file_impl, rename_item_impl, list_directory_impl, delete_item_impl, create_dir_impl,
};

#[test]
fn file_command_impl_roundtrip() {
    let dir = tempfile::tempdir().expect("tempdir should be created");
    let file_path = dir.path().join("notes.txt");
    let file_path_str = file_path.to_string_lossy().to_string();

    write_file_impl(file_path_str.clone(), "hello".to_string()).expect("write should succeed");
    let content = read_file_impl(file_path_str.clone()).expect("read should succeed");
    assert_eq!(content, "hello");

    let renamed = dir.path().join("renamed.txt");
    let renamed_str = renamed.to_string_lossy().to_string();
    rename_item_impl(file_path_str, renamed_str.clone()).expect("rename should succeed");

    let entries = list_directory_impl(dir.path().to_string_lossy().to_string())
        .expect("list should succeed");
    assert!(entries.iter().any(|e| e.name == "renamed.txt"));

    delete_item_impl(renamed_str).expect("delete should succeed");
    let entries_after = list_directory_impl(dir.path().to_string_lossy().to_string())
        .expect("list should succeed");
    assert!(entries_after.is_empty());
}

#[test]
fn create_dir_impl_creates_directory() {
    let dir = tempfile::tempdir().expect("tempdir should be created");
    let subdir = dir.path().join("subdir");
    create_dir_impl(subdir.to_string_lossy().to_string()).expect("mkdir should succeed");
    assert!(subdir.exists());
    assert!(subdir.is_dir());
}
