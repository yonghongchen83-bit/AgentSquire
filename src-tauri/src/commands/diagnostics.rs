#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputEntry {
    pub source: String,
    pub line: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorEntry {
    pub id: String,
    pub message: String,
    pub severity: String,
    pub source: Option<String>,
    pub timestamp: String,
    pub stack_trace: Option<String>,
}

pub fn get_output_impl(_source: String) -> Result<Vec<OutputEntry>, String> {
    Ok(Vec::new())
}

pub fn get_errors_impl() -> Result<Vec<ErrorEntry>, String> {
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_output_returns_empty_list_by_default() {
        let out = get_output_impl("chat".to_string()).expect("get_output should succeed");
        assert!(out.is_empty());
    }

    #[test]
    fn get_errors_returns_empty_list_by_default() {
        let out = get_errors_impl().expect("get_errors should succeed");
        assert!(out.is_empty());
    }
}
