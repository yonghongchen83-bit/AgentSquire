use super::AppState;
use crate::agent::{ToolDanger, ToolRegistry};
use crate::state::config::McpServerConfig;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub category: String, // "system" | "mcp"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_name: Option<String>,
    pub danger: String, // "safe" | "destructive"
    pub enabled: bool,
}

pub async fn list_available_tools(state: State<'_, AppState>) -> Result<Vec<ToolInfo>, String> {
    let (disabled_tools, enabled_servers) = {
        let config = state.config.read().map_err(|e| e.to_string())?;
        let disabled: Vec<String> = config.disabled_tools.clone();
        let servers: Vec<McpServerConfig> = config
            .mcp_servers
            .iter()
            .filter(|s| s.enabled)
            .cloned()
            .collect();
        (disabled, servers)
    };

    // System tools
    let system_registry = ToolRegistry::new();
    let mut tools: Vec<ToolInfo> = system_registry
        .definitions()
        .into_iter()
        .map(|def| {
            let danger = system_registry
                .danger(&def.name)
                .map(|d| match d {
                    ToolDanger::Safe => "safe",
                    ToolDanger::Destructive => "destructive",
                })
                .unwrap_or("safe");
            ToolInfo {
                name: def.name.clone(),
                description: def.description,
                category: "system".to_string(),
                server_name: None,
                danger: danger.to_string(),
                enabled: !disabled_tools.contains(&def.name),
            }
        })
        .collect();

    tools.push(ToolInfo {
        name: "subagent".to_string(),
        description:
            "Spawn a sub-agent to work on a task independently. The sub-agent gets full tool access and reports back when done.".to_string(),
        category: "system".to_string(),
        server_name: None,
        danger: "safe".to_string(),
        enabled: !disabled_tools.contains(&"subagent".to_string()),
    });

    // MCP tools — discover from enabled servers with a short timeout
    for server in &enabled_servers {
        match tokio::time::timeout(
            std::time::Duration::from_secs(5),
            crate::mcp::discover_tools(server.clone()),
        )
        .await
        {
            Ok(Ok(discovered)) => {
                let server_id = server
                    .id
                    .chars()
                    .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                    .collect::<String>();
                for mcp_tool in discovered {
                    let tool_id = mcp_tool
                        .name
                        .chars()
                        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                        .collect::<String>();
                    let local_name = format!("mcp_{}_{}", server_id, tool_id);
                    tools.push(ToolInfo {
                        name: local_name.clone(),
                        description: format!(
                            "MCP tool '{}' from server '{}': {}",
                            mcp_tool.name, server.name, mcp_tool.description
                        ),
                        category: "mcp".to_string(),
                        server_name: Some(server.name.clone()),
                        danger: "destructive".to_string(),
                        enabled: !disabled_tools.contains(&local_name),
                    });
                }
            }
            Ok(Err(_)) | Err(_) => {
                // Server not reachable — skip for now; user can still manage via MCP panel
            }
        }
    }

    Ok(tools)
}
