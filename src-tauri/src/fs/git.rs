

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("Git error: {0}")]
    Lib(String),
    #[error("Not a git repository: {0}")]
    NotRepo(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GitStatus {
    pub path: String,
    pub status: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GitDiff {
    pub path: String,
    pub diff: String,
    pub staged: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GitLogEntry {
    pub hash: String,
    pub author: String,
    pub message: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GitBranch {
    pub name: String,
    pub current: bool,
}

fn open_repo(path: &str) -> Result<git2::Repository, GitError> {
    git2::Repository::open(path)
        .or_else(|_| git2::Repository::discover(path))
        .map_err(|e| GitError::Lib(e.to_string()))
}

pub fn status(path: &str) -> Result<Vec<GitStatus>, GitError> {
    let repo = open_repo(path)?;
    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true);

    let statuses = repo
        .statuses(Some(&mut opts))
        .map_err(|e| GitError::Lib(e.to_string()))?;

    let mut result = Vec::new();
    for entry in statuses.iter() {
        let s = entry.status();
        let label = if s.is_index_new() || s.is_wt_new() {
            "added"
        } else if s.is_index_modified() || s.is_wt_modified() {
            "modified"
        } else if s.is_index_deleted() || s.is_wt_deleted() {
            "deleted"
        } else if s.is_index_renamed() || s.is_wt_renamed() {
            "renamed"
        } else if s.is_index_typechange() || s.is_wt_typechange() {
            "typechange"
        } else if s.is_conflicted() {
            "conflicted"
        } else {
            "unknown"
        };

        result.push(GitStatus {
            path: entry.path().unwrap_or("").to_string(),
            status: label.to_string(),
        });
    }
    Ok(result)
}

pub fn diff(path: &str, staged: bool) -> Result<Vec<GitDiff>, GitError> {
    let repo = open_repo(path)?;
    let tree = repo
        .head()
        .ok()
        .and_then(|h| h.peel_to_tree().ok());

    let diff = if staged {
        let mut opts = git2::DiffOptions::new();
        repo.diff_tree_to_index(tree.as_ref(), None, Some(&mut opts))
    } else {
        let mut opts = git2::DiffOptions::new();
        repo.diff_index_to_workdir(None, Some(&mut opts))
    }
    .map_err(|e| GitError::Lib(e.to_string()))?;

    let mut files: Vec<String> = Vec::new();
    let lines_per_file: std::cell::RefCell<Vec<String>> = std::cell::RefCell::new(Vec::new());

    diff.foreach(
        &mut |file, _| {
            let path = file
                .new_file()
                .path()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            files.push(path);
            lines_per_file.borrow_mut().push(String::new());
            true
        },
        None,
        None,
        Some(&mut |_delta, _hunk, line| {
            if let Some(last) = lines_per_file.borrow_mut().last_mut() {
                let prefix = match line.origin() {
                    '+' => "+",
                    '-' => "-",
                    _ => " ",
                };
                if let Ok(content) = std::str::from_utf8(line.content()) {
                    last.push_str(&format!("{}{}", prefix, content));
                }
            }
            true
        }),
    )
    .map_err(|e| GitError::Lib(e.to_string()))?;

    let results: Vec<GitDiff> = files
        .into_iter()
        .zip(lines_per_file.into_inner().into_iter())
        .map(|(path, diff)| GitDiff { path, diff, staged })
        .collect();

    Ok(results)
}

pub fn log(path: &str, max_count: i32) -> Result<Vec<GitLogEntry>, GitError> {
    let repo = open_repo(path)?;
    let mut revwalk = repo
        .revwalk()
        .map_err(|e| GitError::Lib(e.to_string()))?;
    revwalk
        .push_head()
        .map_err(|e| GitError::Lib(e.to_string()))?;
    revwalk
        .set_sorting(git2::Sort::TIME)
        .map_err(|e| GitError::Lib(e.to_string()))?;

    let mut entries = Vec::new();
    for (_, oid) in revwalk.enumerate().take(max_count as usize) {
        let oid = oid.map_err(|e| GitError::Lib(e.to_string()))?;
        let commit = repo
            .find_commit(oid)
            .map_err(|e| GitError::Lib(e.to_string()))?;
        entries.push(GitLogEntry {
            hash: oid.to_string(),
            author: commit.author().name().unwrap_or("unknown").to_string(),
            message: commit.message().unwrap_or("").trim().to_string(),
            timestamp: commit
                .time()
                .seconds()
                .to_string(),
        });
    }
    Ok(entries)
}

pub fn branches(path: &str) -> Result<Vec<GitBranch>, GitError> {
    let repo = open_repo(path)?;
    let mut result = Vec::new();
    let current_head = repo.head().ok().map(|h| h.shorthand().unwrap_or("").to_string());

    let branches = repo
        .branches(None)
        .map_err(|e| GitError::Lib(e.to_string()))?;

    for branch_result in branches {
        let (branch, _) = branch_result.map_err(|e| GitError::Lib(e.to_string()))?;
        let name = branch
            .name()
            .unwrap_or(Some("unknown"))
            .unwrap_or("unknown")
            .to_string();
        let is_current = current_head.as_deref() == Some(&name);
        result.push(GitBranch {
            name,
            current: is_current,
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
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
        repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[]).unwrap();

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
        assert!(entries.iter().any(|e| e.path == "test.txt"));
    }

    #[test]
    fn test_log() {
        let (_dir, path) = init_temp_repo();
        let entries = log(&path, 10).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].message, "initial");
    }

    #[test]
    fn test_branches() {
        let (_dir, path) = init_temp_repo();
        let branches = branches(&path).unwrap();
        assert!(branches.iter().any(|b| b.current));
    }

    #[test]
    fn test_not_a_repo() {
        let dir = tempfile::tempdir().unwrap();
        let err = status(dir.path().to_str().unwrap());
        assert!(err.is_err());
    }
}
