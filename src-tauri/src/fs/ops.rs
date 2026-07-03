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
