use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use serde_json::Value;
use schemars::JsonSchema;

use crate::common::AppError;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Tool {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Resource {
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolContent {
    #[serde(flatten)]
    pub content: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolResult {
    pub content: Vec<ToolContent>,
    pub is_error: bool,
}

#[async_trait]
pub trait McpClient: Send + Sync {
    /// Connect to the MCP server
    async fn connect(&mut self) -> Result<(), AppError>;

    /// Disconnect from the MCP server
    async fn disconnect(&mut self) -> Result<(), AppError>;

    /// Check if connected
    fn is_connected(&self) -> bool;

    /// List available tools
    async fn list_tools(&mut self) -> Result<Vec<Tool>, AppError>;

    /// Call a tool
    async fn call_tool(
        &mut self,
        name: &str,
        arguments: Value,
    ) -> Result<ToolResult, AppError>;

    /// List available resources
    async fn list_resources(&mut self) -> Result<Vec<Resource>, AppError>;

    /// Read a resource
    async fn read_resource(&mut self, uri: &str) -> Result<Value, AppError>;
}
