pub mod client;

use std::sync::Arc;

use aa_kernel::tool_pack::*;
use async_trait::async_trait;
use client::McpClient;

#[derive(Debug, Clone)]
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
}

struct ConnectedServer {
    config: McpServerConfig,
    client: Arc<McpClient>,
    tools: Vec<ToolDef>,

}

#[derive(Clone)]
struct ToolDef {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

pub struct McpToolPack {
    servers: Vec<ConnectedServer>,
}

impl McpToolPack {
    /// 连接所有配置的 MCP 服务器并发现其工具。
    pub fn new(servers: Vec<McpServerConfig>) -> Self {
        let servers = servers
            .into_iter()
            .filter_map(|config| {
                let rt = match tokio::runtime::Runtime::new() {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::warn!("MCP runtime create: {e}");
                        return None;
                    }
                };

                let client = match rt.block_on(McpClient::connect(&config.command, &config.args))
                {
                    Ok(c) => Arc::new(c),
                    Err(e) => {
                        tracing::warn!("MCP '{}' connect: {e}", config.name);
                        return None;
                    }
                };

                let tools = match rt.block_on(client.list_tools()) {
                    Ok(list) => list
                        .into_iter()
                        .map(|t| ToolDef {
                            name: t.name,
                            description: t.description,
                            input_schema: t.input_schema,
                        })
                        .collect(),
                    Err(e) => {
                        tracing::warn!("MCP '{}' list_tools: {e}", config.name);
                        return None;
                    }
                };

                Some(ConnectedServer {
                    config,
                    client,
                    tools,
                })
            })
            .collect();

        Self { servers }
    }

    pub fn from_json(json: serde_json::Value) -> Self {
        let servers = json
            .as_object()
            .map(|obj| {
                obj.iter()
                    .map(|(name, val)| McpServerConfig {
                        name: name.clone(),
                        command: val["command"]
                            .as_str()
                            .unwrap_or("")
                            .to_owned(),
                        args: val["args"]
                            .as_array()
                            .map(|a| {
                                a.iter()
                                    .filter_map(|v| v.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        Self::new(servers)
    }
}

impl ToolPack for McpToolPack {
    fn tools(&self, _scope: &ToolPackScope<'_>) -> Vec<Arc<dyn Tool>> {
        let mut tools: Vec<Arc<dyn Tool>> = Vec::new();

        for server in &self.servers {
            for def in &server.tools {
                tools.push(Arc::new(McpTool {
                    server_name: server.config.name.clone(),
                    tool_name: def.name.clone(),
                    tool_desc: def.description.clone(),
                    input_schema: def.input_schema.clone(),
                    client: server.client.clone(),
                }));
            }
        }

        tools
    }
}

struct McpTool {
    server_name: String,
    tool_name: String,
    tool_desc: String,
    input_schema: serde_json::Value,
    client: Arc<McpClient>,
}

#[async_trait]
impl Tool for McpTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: format!("{}__{}", self.server_name, self.tool_name),
            description: self.tool_desc.clone(),
            parameters: self.input_schema.clone(),
            origin: ToolOrigin::Subprocess,
            execution_mode: ExecutionMode::Sequential,
        }
    }

    async fn execute(
        &self,
        arguments: serde_json::Value,
        _ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let result = self
            .client
            .call_tool(&self.tool_name, arguments)
            .await
            .map_err(|e| ToolError::Execution(format!("MCP call: {e}")))?;

        Ok(ToolResult {
            content: result.content,
            is_error: result.is_error,
            metadata: None,
        })
    }
}
