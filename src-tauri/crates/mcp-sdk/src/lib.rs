use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerSpec {
    pub id: String,
    pub name: String,
    pub transport: String,
    pub command: String,
    pub args: Vec<String>,
    pub url: Option<String>,
    pub enabled: bool,
    pub env: HashMap<String, String>,
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct DiscoveredTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

struct StdioMcpClient {
    child: Child,
    reader: BufReader<ChildStdout>,
    writer: ChildStdin,
    next_id: u64,
}

impl StdioMcpClient {
    fn connect(server: &McpServerSpec) -> Result<Self, String> {
        if server.transport.to_lowercase() != "stdio" {
            return Err(format!(
                "MCP transport '{}' is not supported for handshake yet",
                server.transport
            ));
        }

        let command = server.command.trim();
        if command.is_empty() {
            return Err(format!("MCP server '{}' has empty command", server.name));
        }

        let mut cmd = Command::new(command);
        cmd.args(&server.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        for (k, v) in &server.env {
            cmd.env(k, v);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to start MCP server '{}': {}", server.name, e))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Failed to capture MCP stdout".to_string())?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Failed to capture MCP stdin".to_string())?;

        Ok(Self {
            child,
            reader: BufReader::new(stdout),
            writer: stdin,
            next_id: 1,
        })
    }

    fn initialize(&mut self) -> Result<(), String> {
        let _ = self.request(
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "squirecli",
                    "version": "0.1.0"
                }
            }),
        )?;

        self.notify("notifications/initialized", json!({}))?;
        Ok(())
    }

    fn list_tools(&mut self) -> Result<Vec<DiscoveredTool>, String> {
        let result = self.request("tools/list", json!({}))?;
        let tools = result["tools"]
            .as_array()
            .ok_or_else(|| "Invalid tools/list response: missing tools array".to_string())?;

        let mut out = Vec::new();
        for t in tools {
            let name = t["name"].as_str().unwrap_or("").trim().to_string();
            if name.is_empty() {
                continue;
            }
            let description = t["description"].as_str().unwrap_or("MCP tool").to_string();
            let input_schema = t
                .get("inputSchema")
                .cloned()
                .or_else(|| t.get("input_schema").cloned())
                .unwrap_or_else(|| json!({"type": "object", "properties": {}}));

            out.push(DiscoveredTool {
                name,
                description,
                input_schema,
            });
        }

        Ok(out)
    }

    fn call_tool(&mut self, name: &str, arguments: Value) -> Result<(String, bool), String> {
        let result = self.request(
            "tools/call",
            json!({
                "name": name,
                "arguments": arguments,
            }),
        )?;

        let is_error = result
            .get("isError")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if let Some(content) = result.get("content").and_then(|v| v.as_array()) {
            let mut text_parts: Vec<String> = Vec::new();
            for item in content {
                if item.get("type").and_then(|v| v.as_str()) == Some("text") {
                    if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                        text_parts.push(text.to_string());
                    }
                }
            }
            if !text_parts.is_empty() {
                return Ok((text_parts.join("\n"), is_error));
            }
        }

        Ok((
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string()),
            is_error,
        ))
    }

    fn notify(&mut self, method: &str, params: Value) -> Result<(), String> {
        let msg = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        self.write_message(&msg)
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;

        let msg = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        self.write_message(&msg)?;

        loop {
            let response = self.read_message()?;
            if response.get("id").and_then(|v| v.as_u64()) != Some(id) {
                continue;
            }

            if let Some(err) = response.get("error") {
                let message = err
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("MCP error");
                return Err(message.to_string());
            }

            return response
                .get("result")
                .cloned()
                .ok_or_else(|| "Missing result in MCP response".to_string());
        }
    }

    fn write_message(&mut self, msg: &Value) -> Result<(), String> {
        let payload = serde_json::to_vec(msg).map_err(|e| e.to_string())?;
        let header = format!("Content-Length: {}\r\n\r\n", payload.len());
        self.writer
            .write_all(header.as_bytes())
            .map_err(|e| format!("Failed to write MCP header: {}", e))?;
        self.writer
            .write_all(&payload)
            .map_err(|e| format!("Failed to write MCP payload: {}", e))?;
        self.writer
            .flush()
            .map_err(|e| format!("Failed to flush MCP payload: {}", e))
    }

    fn read_message(&mut self) -> Result<Value, String> {
        let mut content_length: Option<usize> = None;
        loop {
            let mut line = String::new();
            let n = self
                .reader
                .read_line(&mut line)
                .map_err(|e| format!("Failed reading MCP header: {}", e))?;
            if n == 0 {
                return Err("MCP stream closed".to_string());
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                break;
            }

            if trimmed.to_ascii_lowercase().starts_with("content-length:") {
                let value = trimmed.split(':').nth(1).unwrap_or("").trim();
                let parsed = value
                    .parse::<usize>()
                    .map_err(|e| format!("Invalid Content-Length '{}': {}", value, e))?;
                content_length = Some(parsed);
            }
        }

        let len = content_length.ok_or_else(|| "Missing Content-Length header".to_string())?;
        let mut buf = vec![0u8; len];
        self.reader
            .read_exact(&mut buf)
            .map_err(|e| format!("Failed reading MCP payload: {}", e))?;

        serde_json::from_slice::<Value>(&buf)
            .map_err(|e| format!("Invalid MCP JSON payload: {}", e))
    }
}

impl Drop for StdioMcpClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub async fn discover_tools(server: McpServerSpec) -> Result<Vec<DiscoveredTool>, String> {
    let task = tokio::task::spawn_blocking(move || {
        let mut client = StdioMcpClient::connect(&server)?;
        client.initialize()?;
        client.list_tools()
    });
    match tokio::time::timeout(Duration::from_secs(15), task).await {
        Ok(join) => join.map_err(|e| format!("MCP discovery join error: {}", e))?,
        Err(_) => Err("MCP discovery timed out after 15s — server did not respond".to_string()),
    }
}

pub async fn call_tool(
    server: McpServerSpec,
    name: String,
    arguments: Value,
) -> Result<(String, bool), String> {
    let task = tokio::task::spawn_blocking(move || {
        let mut client = StdioMcpClient::connect(&server)?;
        client.initialize()?;
        client.call_tool(&name, arguments)
    });
    match tokio::time::timeout(Duration::from_secs(30), task).await {
        Ok(join) => join.map_err(|e| format!("MCP call join error: {}", e))?,
        Err(_) => Err("MCP tool call timed out after 30s — server stopped responding".to_string()),
    }
}
