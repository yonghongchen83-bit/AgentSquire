use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum FsError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Path not found: {0}")]
    NotFound(String),
    #[error("Path is not a file: {0}")]
    IsNotFile(String),
    #[error("Path is not a directory: {0}")]
    IsNotDirectory(String),
    #[error("Already exists: {0}")]
    AlreadyExists(String),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
}

pub fn read_file(path: &str) -> Result<String, FsError> {
    let p = Path::new(path);
    if !p.exists() {
        return Err(FsError::NotFound(path.to_string()));
    }
    if !p.is_file() {
        return Err(FsError::IsNotFile(path.to_string()));
    }
    Ok(std::fs::read_to_string(p)?)
}

pub fn write_file(path: &str, content: &str) -> Result<(), FsError> {
    let p = Path::new(path);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(std::fs::write(p, content)?)
}

pub fn create_dir(path: &str) -> Result<(), FsError> {
    let p = Path::new(path);
    if p.exists() {
        return Err(FsError::AlreadyExists(path.to_string()));
    }
    Ok(std::fs::create_dir_all(p)?)
}

pub fn delete_item(path: &str) -> Result<(), FsError> {
    let p = Path::new(path);
    if !p.exists() {
        return Err(FsError::NotFound(path.to_string()));
    }
    if p.is_dir() {
        std::fs::remove_dir_all(p)?;
    } else {
        std::fs::remove_file(p)?;
    }
    Ok(())
}

pub fn rename_item(from: &str, to: &str) -> Result<(), FsError> {
    Ok(std::fs::rename(from, to)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn unique_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("squirecli-test-{}", name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
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
        fs::write(&a, "a").unwrap();
        fs::write(&b, "b").unwrap();
        fs::create_dir(&sub).unwrap();

        let entries = list_directory(dir.to_str().unwrap()).unwrap();
        assert_eq!(entries.len(), 3);
        assert!(entries[0].is_dir);
        assert_eq!(entries[0].name, "sub");
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
}

pub fn list_directory(path: &str) -> Result<Vec<FileEntry>, FsError> {
    let p = Path::new(path);
    if !p.exists() {
        return Err(FsError::NotFound(path.to_string()));
    }
    if !p.is_dir() {
        return Err(FsError::IsNotDirectory(path.to_string()));
    }
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(p)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        let name = entry.file_name().to_string_lossy().to_string();
        let is_symlink = ft.is_symlink();
        entries.push(FileEntry {
            name,
            path: entry.path().to_string_lossy().to_string(),
            is_dir: ft.is_dir(),
            is_symlink,
            size: if ft.is_file() || is_symlink {
                entry.metadata()?.len()
            } else {
                0
            },
        });
    }
    entries.sort_by(|a, b| {
        if a.is_dir != b.is_dir {
            b.is_dir.cmp(&a.is_dir)
        } else {
            a.name.cmp(&b.name)
        }
    });
    Ok(entries)
}
