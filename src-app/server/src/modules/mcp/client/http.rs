use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

use super::traits::{McpClient, Tool, Resource, ToolResult};
use crate::common::AppError;
use crate::modules::mcp::models::{McpServer, TransportType};

pub struct HttpMcpClient {
    #[allow(dead_code)] // Kept for debugging/logging (future use)
    server_id: Uuid,
    #[allow(dead_code)] // Kept for potential reconfiguration (future use)
    server_config: McpServer,
    client: Client,
    base_url: String,
    connected: bool,
    session_id: Arc<RwLock<Option<String>>>,
}

impl HttpMcpClient {
    pub fn new(server: McpServer) -> Result<Self, AppError> {
        if server.transport_type != TransportType::Http {
            return Err(AppError::bad_request("INVALID_TRANSPORT", "Only HTTP transport supported"));
        }

        let base_url = server.url.clone()
            .ok_or_else(|| AppError::bad_request("MISSING_URL", "Missing URL for HTTP transport"))?;

        let mut client_builder = Client::builder()
            .timeout(std::time::Duration::from_secs(
                server.timeout_seconds.max(1) as u64
            ));

        // Add custom headers if provided
        if let Some(headers_map) = server.headers.as_object() {
            let mut headers = reqwest::header::HeaderMap::new();
            for (key, value) in headers_map {
                if let Some(val_str) = value.as_str() {
                    if let (Ok(name), Ok(val)) = (
                        reqwest::header::HeaderName::from_bytes(key.as_bytes()),
                        reqwest::header::HeaderValue::from_str(val_str)
                    ) {
                        headers.insert(name, val);
                    }
                }
            }
            client_builder = client_builder.default_headers(headers);
        }

        Ok(Self {
            server_id: server.id,
            server_config: server,
            client: client_builder.build()
                .map_err(|e| AppError::internal_error(format!("Failed to create HTTP client: {}", e)))?,
            base_url,
            connected: false,
            session_id: Arc::new(RwLock::new(None)),
        })
    }

    async fn request<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: Value,
    ) -> Result<T, AppError> {
        let request_body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params
        });

        // Use /mcp endpoint for Streamable HTTP protocol
        let url = if self.base_url.ends_with('/') {
            format!("{}mcp", self.base_url)
        } else {
            format!("{}/mcp", self.base_url)
        };

        let mut request = self.client
            .post(&url)
            .header("Accept", "application/json, text/event-stream")
            .json(&request_body);

        // Add session ID if available
        if let Ok(session_guard) = self.session_id.read() {
            if let Some(ref session_id) = *session_guard {
                request = request.header("mcp-session-id", session_id);
            }
        }

        let response = request.send()
            .await
            .map_err(|e| AppError::internal_error(format!("HTTP request failed: {}", e)))?;

        // Extract session ID from response headers if present
        if let Some(session_id) = response.headers().get("mcp-session-id") {
            if let Ok(session_str) = session_id.to_str() {
                if let Ok(mut session_guard) = self.session_id.write() {
                    *session_guard = Some(session_str.to_string());
                }
            }
        }

        // Get response text and parse SSE format
        let response_text = response.text().await
            .map_err(|e| AppError::internal_error(format!("Failed to get response text: {}", e)))?;

        // Parse SSE format: extract JSON from "data: {...}" lines
        let response_json: Value = if response_text.starts_with("event:") {
            // SSE format - extract data line
            let mut found_data = None;
            for line in response_text.lines() {
                if let Some(data) = line.strip_prefix("data: ") {
                    found_data = Some(serde_json::from_str(data)
                        .map_err(|e| AppError::internal_error(format!("Failed to parse SSE data: {}", e)))?);
                    break;
                }
            }
            found_data.ok_or_else(|| AppError::internal_error("No data found in SSE response"))?
        } else {
            // Plain JSON format
            serde_json::from_str(&response_text)
                .map_err(|e| AppError::internal_error(format!("Failed to parse response: {}", e)))?
        };

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
impl McpClient for HttpMcpClient {
    async fn connect(&mut self) -> Result<(), AppError> {
        // For HTTP, test connectivity with initialize
        let _: Value = self.request("initialize", serde_json::json!({
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

        let result: ListToolsResult = self.request("tools/list", serde_json::json!({})).await?;
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

        let result: ToolResult = self.request("tools/call", serde_json::json!({
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

        let result: ListResourcesResult = self.request("resources/list", serde_json::json!({})).await?;
        Ok(result.resources)
    }

    async fn read_resource(&mut self, uri: &str) -> Result<Value, AppError> {
        if !self.is_connected() {
            return Err(AppError::internal_error("Not connected"));
        }

        let result: Value = self.request("resources/read", serde_json::json!({
            "uri": uri
        })).await?;

        Ok(result)
    }
}
