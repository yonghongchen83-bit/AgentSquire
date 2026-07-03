pub fn normalize_level(level: Option<String>) -> String {
    let raw = level.unwrap_or_else(|| "mid".to_string()).to_lowercase();
    match raw.as_str() {
        "none" | "low" | "mid" | "high" => raw,
        _ => "mid".to_string(),
    }
}

