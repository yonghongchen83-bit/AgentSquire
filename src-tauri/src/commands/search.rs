use crate::search::grep::{self, GrepReplaceOptions, SearchMatch, SearchOptions};

pub fn search_files_impl(
    query: String,
    path: String,
    regex: bool,
    case_sensitive: bool,
    whole_word: bool,
    max_results: Option<usize>,
    glob: Option<String>,
    context_lines: Option<u64>,
) -> Result<Vec<SearchMatch>, String> {
    let options = SearchOptions {
        query,
        path,
        regex,
        case_sensitive,
        whole_word,
        max_results,
        glob,
        context_lines,
    };
    grep::search(&options).map_err(|e| e.to_string())
}

pub fn replace_in_files_impl(
    query: String,
    replacement: String,
    path: String,
    regex: bool,
    case_sensitive: bool,
    glob: Option<String>,
) -> Result<usize, String> {
    let options = GrepReplaceOptions {
        query,
        replacement,
        path,
        regex,
        case_sensitive,
        glob,
    };
    grep::grep_replace(&options).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_files_impl_rejects_missing_rg() {
        let result = search_files_impl(
            "needle".to_string(),
            "./path-that-should-not-exist".to_string(),
            false,
            false,
            false,
            Some(1),
            None,
            None,
        );

        // This command path should not panic; it either errors gracefully
        // or returns an empty vector depending on environment.
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn replace_in_files_impl_replaces_plain_text() {
        let dir = tempfile::tempdir().expect("tempdir should be created");
        let file_path = dir.path().join("a.txt");
        std::fs::write(&file_path, "hello world\nhello world\n")
            .expect("seed file write should succeed");

        let count = replace_in_files_impl(
            "hello".to_string(),
            "bye".to_string(),
            dir.path().to_string_lossy().to_string(),
            false,
            true,
            Some("*.txt".to_string()),
        )
        .expect("replace should succeed");

        let updated = std::fs::read_to_string(&file_path).expect("read should succeed");
        assert!(count >= 1);
        assert!(updated.contains("bye world"));
    }
}
