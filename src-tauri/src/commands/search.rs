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
