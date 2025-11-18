use async_trait::async_trait;
use eventsource_client as es;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::traits::{McpClient, Tool, Resource, ToolResult};
use crate::common::AppError;
use crate::modules::mcp::models::{McpServer, TransportType};

pub struct SseMcpClient {
    #[allow(dead_code)] // Kept for debugging/logging (future use)
    server_id: Uuid,
    #[allow(dead_code)] // Kept for potential reconfiguration (future use)
    server_config: McpServer,
    base_url: String,
    event_source: Option<Arc<RwLock<dyn es::Client>>>,
    connected: bool,
}

impl SseMcpClient {
    pub fn new(server: McpServer) -> Result<Self, AppError> {
        if server.transport_type != TransportType::Sse {
            return Err(AppError::bad_request("INVALID_TRANSPORT", "Only SSE transport supported"));
        }

        let base_url = server.url.clone()
            .ok_or_else(|| AppError::bad_request("MISSING_URL", "Missing URL for SSE transport"))?;

        Ok(Self {
            server_id: server.id,
            server_config: server,
            base_url,
            event_source: None,
            connected: false,
        })
    }

    async fn send_request<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: Value,
    ) -> Result<T, AppError> {
        // SSE typically uses HTTP POST for requests
        let client = reqwest::Client::new();

        let mut request = client.post(format!("{}/rpc", self.base_url))
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": method,
                "params": params
            }));

        // Add custom headers if provided
        if let Some(headers_map) = self.server_config.headers.as_object() {
            for (key, value) in headers_map {
                if let Some(val_str) = value.as_str() {
                    request = request.header(key, val_str);
                }
            }
        }

        let response = request.send().await
            .map_err(|e| AppError::internal_error(format!("SSE request failed: {}", e)))?;

        let response_json: Value = response.json().await
            .map_err(|e| AppError::internal_error(format!("Failed to parse response: {}", e)))?;

        if let Some(error) = response_json.get("error") {
            return Err(AppError::internal_error(format!("MCP error: {}", error)));
        }

        let result = response_json.get("result")
            .ok_or_else(|| AppError::internal_error("Missing result in response"))?;

        serde_json::from_value(result.clone())
            .map_err(|e| AppError::internal_error(format!("Failed to deserialize result: {}", e)))
    }
}

#[async_trait]
impl McpClient for SseMcpClient {
    async fn connect(&mut self) -> Result<(), AppError> {
        // Create SSE connection for events
        let mut builder = es::ClientBuilder::for_url(&self.base_url)
            .map_err(|e| AppError::internal_error(format!("Failed to create SSE client: {}", e)))?;

        // Add headers if provided
        if let Some(headers_map) = self.server_config.headers.as_object() {
            for (key, value) in headers_map {
                if let Some(val_str) = value.as_str() {
                    builder = builder.header(key, val_str)
                        .map_err(|e| AppError::internal_error(format!("Failed to add header: {}", e)))?;
                }
            }
        }

        let client = builder.build();
        self.event_source = Some(Arc::new(RwLock::new(client)));

        // Test connection with initialize
        let _: Value = self.send_request("initialize", serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "ziee-chat",
                "version": env!("CARGO_PKG_VERSION")
            }
        })).await?;

        self.connected = true;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), AppError> {
        self.event_source = None;
        self.connected = false;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    async fn list_tools(&mut self) -> Result<Vec<Tool>, AppError> {
        if !self.is_connected() {
            return Err(AppError::internal_error("Not connected"));
        }

        #[derive(serde::Deserialize)]
        struct ListToolsResult {
            tools: Vec<Tool>,
        }

        let result: ListToolsResult = self.send_request("tools/list", serde_json::json!({})).await?;
        Ok(result.tools)
    }

    async fn call_tool(
        &mut self,
        name: &str,
        arguments: Value,
    ) -> Result<ToolResult, AppError> {
        if !self.is_connected() {
            return Err(AppError::internal_error("Not connected"));
        }

        let result: ToolResult = self.send_request("tools/call", serde_json::json!({
            "name": name,
            "arguments": arguments
        })).await?;

        Ok(result)
    }

    async fn list_resources(&mut self) -> Result<Vec<Resource>, AppError> {
        if !self.is_connected() {
            return Err(AppError::internal_error("Not connected"));
        }

        #[derive(serde::Deserialize)]
        struct ListResourcesResult {
            resources: Vec<Resource>,
        }

        let result: ListResourcesResult = self.send_request("resources/list", serde_json::json!({})).await?;
        Ok(result.resources)
    }

    async fn read_resource(&mut self, uri: &str) -> Result<Value, AppError> {
        if !self.is_connected() {
            return Err(AppError::internal_error("Not connected"));
        }

        let result: Value = self.send_request("resources/read", serde_json::json!({
            "uri": uri
        })).await?;

        Ok(result)
    }
}
