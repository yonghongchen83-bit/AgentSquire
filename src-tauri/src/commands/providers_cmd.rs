use super::AppState;
use crate::llm::registry::ProviderInfo;
use crate::state::config::McpServerConfig;
use tauri::State;

fn openai_chat_url(base: String) -> String {
    if base.ends_with("/chat/completions") || base.ends_with("/responses") {
        base
    } else {
        format!("{}/chat/completions", base.trim_end_matches('/'))
    }
}

fn anthropic_messages_url(base: String) -> String {
    if base.ends_with("/messages") {
        base
    } else {
        format!("{}/messages", base.trim_end_matches('/'))
    }
}

fn derive_models_base_url(endpoint: &str) -> String {
    let trimmed = endpoint.trim_end_matches('/');
    if trimmed.ends_with("/chat/completions")
        || trimmed.ends_with("/responses")
        || trimmed.ends_with("/messages")
    {
        trimmed
            .rsplit_once('/')
            .map(|(base, _)| base)
            .unwrap_or(trimmed)
            .to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn list_providers_impl(state: State<'_, AppState>) -> Vec<ProviderInfo> {
    state
        .registry
        .read()
        .map(|reg| reg.list())
        .unwrap_or_default()
}

pub async fn test_connection_impl(
    provider_type: String,
    api_key: String,
    model: String,
    endpoint: Option<String>,
) -> Result<String, String> {
    if api_key.is_empty() {
        return Err("API key is required".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|_| "Connection failed: unable to create HTTP client".to_string())?;

    match provider_type.to_lowercase().as_str() {
        "openai" | "openrouter" => {
            let base = endpoint.unwrap_or_else(|| "https://api.openai.com/v1".to_string());
            let url = openai_chat_url(base);

            let body = serde_json::json!({
                "model": model,
                "messages": [{"role": "user", "content": "Say ok"}],
                "max_tokens": 50,
                "stream": false,
            });

            let resp = client
                .post(&url)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|e| {
                    let msg = e.to_string();
                    if msg.contains("dns")
                        || msg.contains("resolve")
                        || msg.contains("connect")
                        || msg.contains("refused")
                        || msg.contains("timed out")
                    {
                        "Connection failed: unable to reach the server".to_string()
                    } else {
                        format!("Connection failed: {}", msg)
                    }
                })?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body_text = resp.text().await.unwrap_or_default();
                let detail = if body_text.is_empty() {
                    String::new()
                } else {
                    let trimmed = body_text.trim();
                    let snippet = if trimmed.len() > 300 {
                        &trimmed[..300]
                    } else {
                        trimmed
                    };
                    format!(": {}", snippet)
                };
                return match status.as_u16() {
                    401 => Err(format!(
                        "Connection failed: invalid API key or authentication error{}",
                        detail
                    )),
                    429 => Err(format!(
                        "Connection failed: rate limited by the server{}",
                        detail
                    )),
                    _ => Err(format!("Connection failed: HTTP {}{}", status, detail)),
                };
            }

            let json: serde_json::Value = resp
                .json()
                .await
                .map_err(|_| "Connection failed: invalid response from server".to_string())?;

            let content = json["choices"][0]["message"]["content"].as_str().unwrap_or("");
            let finish = json["choices"][0]["finish_reason"].as_str().unwrap_or("");

            if finish == "stop" || finish == "length" {
                Ok("Connection successful".to_string())
            } else {
                Ok(format!("Connected (response: {})", content))
            }
        }
        "anthropic" => {
            let base = endpoint.unwrap_or_else(|| "https://api.anthropic.com/v1".to_string());
            let url = anthropic_messages_url(base);

            let body = serde_json::json!({
                "model": model,
                "max_tokens": 50,
                "messages": [{"role": "user", "content": "Say ok"}],
            });

            let resp = client
                .post(&url)
                .header("x-api-key", &api_key)
                .header("anthropic-version", "2023-06-01")
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|_| "Connection failed: unable to reach the server".to_string())?;

            if !resp.status().is_success() {
                let status = resp.status();
                return match status.as_u16() {
                    401 => {
                        Err("Connection failed: invalid API key or authentication error"
                            .to_string())
                    }
                    429 => Err("Connection failed: rate limited by the server".to_string()),
                    _ => Err(format!("Connection failed: HTTP {}", status)),
                };
            }

            Ok("Connection successful".to_string())
        }
        _ => Err(format!("Unknown provider type: {}", provider_type)),
    }
}

pub async fn fetch_models_impl(
    provider_type: String,
    endpoint: String,
    api_key: Option<String>,
) -> Result<Vec<String>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let models_url = derive_models_base_url(&endpoint);

    match provider_type.to_lowercase().as_str() {
        "openai" | "openrouter" | "custom" => {
            let url = format!("{}/models", models_url);
            let mut req = client.get(&url);
            if let Some(key) = &api_key {
                req = req.header("Authorization", format!("Bearer {}", key));
            }
            let resp = req.send().await.map_err(|e| format!("HTTP error: {}", e))?;
            let status = resp.status();
            let text = resp.text().await.map_err(|e| e.to_string())?;
            if !status.is_success() {
                return Err(format!("Server returned {}: {}", status, text));
            }
            let json: serde_json::Value =
                serde_json::from_str(&text).map_err(|e| format!("Invalid JSON: {}", e))?;
            let models = json["data"]
                .as_array()
                .ok_or_else(|| "No 'data' array in response".to_string())?;
            let names: Vec<String> = models
                .iter()
                .filter_map(|m| m["id"].as_str().map(|s| s.to_string()))
                .collect();
            if names.is_empty() {
                return Err("No models found in response".to_string());
            }
            Ok(names)
        }
        "anthropic" => {
            let url = format!("{}/models", models_url);
            let mut req = client.get(&url);
            if let Some(key) = &api_key {
                req = req.header("x-api-key", key);
                req = req.header("anthropic-version", "2023-06-01");
            }
            let resp = req.send().await.map_err(|e| format!("HTTP error: {}", e))?;
            let status = resp.status();
            let text = resp.text().await.map_err(|e| e.to_string())?;
            if !status.is_success() {
                return Err(format!("Server returned {}: {}", status, text));
            }
            let json: serde_json::Value =
                serde_json::from_str(&text).map_err(|e| format!("Invalid JSON: {}", e))?;
            let models = json["data"]
                .as_array()
                .ok_or_else(|| "No 'data' array in response".to_string())?;
            let names: Vec<String> = models
                .iter()
                .filter_map(|m| m["id"].as_str().map(|s| s.to_string()))
                .collect();
            if names.is_empty() {
                return Err("No models found in response".to_string());
            }
            Ok(names)
        }
        _ => Err(format!("Unknown provider type: {}", provider_type)),
    }
}

pub async fn test_mcp_connection_impl(server: McpServerConfig) -> Result<String, String> {
    let transport = server.transport.to_lowercase();

    if transport == "stdio" {
        let cmd = server.command.trim();
        if cmd.is_empty() {
            return Err("Local MCP command is required".to_string());
        }

        let path = std::path::Path::new(cmd);
        if path.is_absolute() {
            if path.exists() {
                return Ok(format!("Local MCP command found: {}", cmd));
            }
            return Err(format!("Local MCP command not found: {}", cmd));
        }

        #[cfg(windows)]
        let checker = "where";
        #[cfg(not(windows))]
        let checker = "which";

        let output = std::process::Command::new(checker)
            .arg(cmd)
            .output()
            .map_err(|e| format!("Failed to run command lookup: {}", e))?;

        if output.status.success() {
            return Ok(format!("Local MCP command is available in PATH: {}", cmd));
        }

        return Err(format!("Local MCP command is not in PATH: {}", cmd));
    }

    if transport == "http" || transport == "sse" {
        let url = server
            .url
            .as_ref()
            .map(|u| u.trim())
            .filter(|u| !u.is_empty())
            .ok_or_else(|| "Remote MCP URL is required".to_string())?;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        let mut req = client.get(url);
        for (k, v) in &server.headers {
            req = req.header(k, v);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| format!("Failed to reach remote MCP server: {}", e))?;

        return Ok(format!("Remote MCP endpoint reachable (HTTP {})", resp.status()));
    }

    Err(format!("Unknown MCP transport: {}", transport))
}

#[cfg(test)]
#[path = "providers_cmd_test.rs"]
mod tests;
