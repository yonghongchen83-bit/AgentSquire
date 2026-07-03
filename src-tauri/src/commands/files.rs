use crate::fs::ops::{self, FileEntry};

pub fn read_file_impl(path: String) -> Result<String, String> {
    ops::read_file(&path).map_err(|e| e.to_string())
}

pub fn write_file_impl(path: String, content: String) -> Result<(), String> {
    ops::write_file(&path, &content).map_err(|e| e.to_string())
}

pub fn create_dir_impl(path: String) -> Result<(), String> {
    ops::create_dir(&path).map_err(|e| e.to_string())
}

pub fn delete_item_impl(path: String) -> Result<(), String> {
    ops::delete_item(&path).map_err(|e| e.to_string())
}

pub fn rename_item_impl(from: String, to: String) -> Result<(), String> {
    ops::rename_item(&from, &to).map_err(|e| e.to_string())
}

pub fn list_directory_impl(path: String) -> Result<Vec<FileEntry>, String> {
    ops::list_directory(&path).map_err(|e| e.to_string())
}
