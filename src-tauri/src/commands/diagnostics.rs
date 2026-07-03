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
