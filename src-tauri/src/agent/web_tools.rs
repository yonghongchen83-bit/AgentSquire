use async_trait::async_trait;
use serde_json::Value;

use super::{Tool, ToolResult};

pub struct WebFetchTool;

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }

    fn description(&self) -> &str {
        "Fetch a web page and return its HTML content. Useful for reading documentation, checking APIs, or scraping web content."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "URL to fetch"
                },
                "max_length": {
                    "type": "number",
                    "description": "Maximum number of characters to return (default: 10000)"
                }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, call_id: &str, args: Value) -> ToolResult {
        let url = match args.get("url").and_then(|v| v.as_str()) {
            Some(u) => u,
            None => {
                return ToolResult {
                    call_id: call_id.to_string(),
                    output: "Missing required argument: url".to_string(),
                    is_error: true,
                }
            }
        };

        let max_length = args
            .get("max_length")
            .and_then(|v| v.as_u64())
            .unwrap_or(10000) as usize;

        let client = match reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .timeout(std::time::Duration::from_secs(30))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                return ToolResult {
                    call_id: call_id.to_string(),
                    output: format!("Failed to create HTTP client: {}", e),
                    is_error: true,
                }
            }
        };

        match client.get(url).send().await {
            Ok(response) => {
                let status = response.status();
                if !status.is_success() {
                    return ToolResult {
                        call_id: call_id.to_string(),
                        output: format!(
                            "HTTP {} {}: {}",
                            status.as_u16(),
                            status.canonical_reason().unwrap_or("Unknown"),
                            url
                        ),
                        is_error: true,
                    };
                }
                match response.text().await {
                    Ok(body) => {
                        let truncated = if body.len() > max_length {
                            format!(
                                "{}...\n\n[Response truncated to {} characters]",
                                &body[..max_length],
                                max_length
                            )
                        } else {
                            body
                        };
                        ToolResult {
                            call_id: call_id.to_string(),
                            output: format!("Status: {}\n\n{}", status.as_u16(), truncated),
                            is_error: false,
                        }
                    }
                    Err(e) => ToolResult {
                        call_id: call_id.to_string(),
                        output: format!("Failed to read response body: {}", e),
                        is_error: true,
                    },
                }
            }
            Err(e) => ToolResult {
                call_id: call_id.to_string(),
                output: format!("Request failed: {}", e),
                is_error: true,
            },
        }
    }
}
