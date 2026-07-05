use serde_json::Value;

use crate::state::config::McpServerConfig;

pub use mcp_sdk::DiscoveredTool;

fn to_spec(server: McpServerConfig) -> mcp_sdk::McpServerSpec {
    mcp_sdk::McpServerSpec {
        id: server.id,
        name: server.name,
        transport: server.transport,
        command: server.command,
        args: server.args,
        url: server.url,
        enabled: server.enabled,
        env: server.env,
        headers: server.headers,
    }
}

pub async fn discover_tools(server: McpServerConfig) -> Result<Vec<DiscoveredTool>, String> {
    mcp_sdk::discover_tools(to_spec(server)).await
}

pub async fn call_tool(
    server: McpServerConfig,
    name: String,
    arguments: Value,
) -> Result<(String, bool), String> {
    mcp_sdk::call_tool(to_spec(server), name, arguments).await
}
