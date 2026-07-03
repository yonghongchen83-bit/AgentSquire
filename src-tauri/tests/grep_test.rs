use squirecli_lib::search::grep::{SearchOptions, GrepReplaceOptions, SearchError, SearchMatch};

#[test]
fn test_search_options_builder() {
    let opts = SearchOptions {
        query: "fn main".into(),
        path: ".".into(),
        regex: true,
        case_sensitive: false,
        whole_word: true,
        max_results: Some(100),
        glob: Some("*.rs".into()),
        context_lines: Some(3),
    };
    assert_eq!(opts.query, "fn main");
    assert!(opts.regex);
    assert!(!opts.case_sensitive);
    assert!(opts.whole_word);
    assert_eq!(opts.max_results, Some(100));
}

#[test]
fn test_grep_replace_options() {
    let opts = GrepReplaceOptions {
        query: "foo".into(),
        replacement: "bar".into(),
        path: "./src".into(),
        regex: false,
        case_sensitive: true,
        glob: Some("*.rs".into()),
    };
    assert_eq!(opts.query, "foo");
    assert_eq!(opts.replacement, "bar");
}

#[test]
fn test_search_error_display() {
    let err = SearchError::RgNotFound;
    assert_eq!(err.to_string(), "rg binary not found");
    let err = SearchError::Regex(regex::Error::Syntax("bad pattern".into()));
    assert!(err.to_string().contains("bad pattern"));
}

#[test]
fn test_search_match_struct() {
    let m = SearchMatch {
        file: "src/main.rs".into(),
        line_number: 10,
        column: 5,
        content: "fn main() {}".into(),
        context_before: vec![],
        context_after: vec![],
    };
    assert_eq!(m.file, "src/main.rs");
    assert_eq!(m.line_number, 10);
    assert_eq!(m.column, 5);
}
