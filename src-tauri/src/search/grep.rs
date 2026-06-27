#[cfg(test)]
mod tests {
    use super::*;

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
}

use std::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),
    #[error("rg binary not found")]
    RgNotFound,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SearchMatch {
    pub file: String,
    pub line_number: u64,
    pub column: u64,
    pub content: String,
    pub context_before: Vec<String>,
    pub context_after: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SearchOptions {
    pub query: String,
    pub path: String,
    pub regex: bool,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub max_results: Option<usize>,
    pub glob: Option<String>,
    pub context_lines: Option<u64>,
}

pub fn search(options: &SearchOptions) -> Result<Vec<SearchMatch>, SearchError> {
    let mut cmd = Command::new("rg");
    cmd.arg("--json")
        .arg("--no-heading")
        .arg("--line-number")
        .arg("--column");

    if !options.regex {
        cmd.arg("--fixed-strings");
    }
    if !options.case_sensitive {
        cmd.arg("-i");
    }
    if options.whole_word {
        cmd.arg("-w");
    }
    if let Some(max) = options.max_results {
        cmd.arg("-m").arg(max.to_string());
    }
    if let Some(glob) = &options.glob {
        cmd.arg("--glob").arg(glob);
    }
    if let Some(ctx) = options.context_lines {
        cmd.arg("-C").arg(ctx.to_string());
    }

    cmd.arg(&options.query).arg(&options.path);

    let output = cmd.output()?;
    if !output.status.success() && !output.stderr.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("command not found") || stderr.contains("not found") {
            return Err(SearchError::RgNotFound);
        }
    }

    let mut results: Vec<SearchMatch> = Vec::new();
    let mut context_before: Vec<String> = Vec::new();
    let mut pending_file: Option<String> = None;

    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
            let type_ = val["type"].as_str().unwrap_or("");
            match type_ {
                "begin" => {
                    pending_file = val["data"]["path"]["text"].as_str().map(|s| s.to_string());
                    context_before.clear();
                }
                "match" => {
                    let file = pending_file
                        .clone()
                        .or_else(|| {
                            val["data"]["path"]["text"].as_str().map(|s| s.to_string())
                        })
                        .unwrap_or_default();
                    let line_number = val["data"]["line_number"].as_u64().unwrap_or(0);
                    let column = val["data"]["absolute_column"].as_u64().unwrap_or(0);
                    let content = val["data"]["lines"]["text"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();

                    results.push(SearchMatch {
                        file,
                        line_number,
                        column,
                        content,
                        context_before: context_before.clone(),
                        context_after: Vec::new(),
                    });
                    context_before.clear();
                }
                "context" => {
                    let text = val["data"]["lines"]["text"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();
                    context_before.push(text);
                }
                "summary" => {}
                _ => {}
            }
        }
    }

    Ok(results)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GrepReplaceOptions {
    pub query: String,
    pub replacement: String,
    pub path: String,
    pub regex: bool,
    pub case_sensitive: bool,
    pub glob: Option<String>,
}

pub fn grep_replace(options: &GrepReplaceOptions) -> Result<usize, SearchError> {
    let mut cmd = Command::new("rg");
    cmd.arg("--json")
        .arg("--line-number")
        .arg("--column");

    if !options.regex {
        cmd.arg("--fixed-strings");
    }
    if !options.case_sensitive {
        cmd.arg("-i");
    }
    if let Some(glob) = &options.glob {
        cmd.arg("--glob").arg(glob);
    }

    cmd.arg(&options.query).arg(&options.path);

    let output = cmd.output()?;
    let mut count = 0;

    for line_result in String::from_utf8_lossy(&output.stdout).lines() {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(line_result) {
            if val["type"] == "match" {
                count += 1;
                let file = val["data"]["path"]["text"].as_str().unwrap_or("");
                if file.is_empty() {
                    continue;
                }
                let content = std::fs::read_to_string(file).ok();
                if let Some(ref content) = content {
                    let new_content = if options.regex {
                        let re = regex::Regex::new(&options.query)?;
                        re.replace_all(content, options.replacement.as_str())
                            .to_string()
                    } else {
                        content.replace(&options.query, &options.replacement)
                    };
                    std::fs::write(file, &new_content).ok();
                }
            }
        }
    }

    Ok(count)
}
