use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::Value;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use super::traits::{McpClient, Tool, Resource, ToolResult};
use crate::common::AppError;
use crate::modules::mcp::models::{McpServer, TransportType};
use crate::modules::mcp::sampling::SamplingHandler;

/// Maximum bytes for a single buffered SSE event from the MCP server.
/// 50 MB is generous enough for long prompt-enhancement payloads while still
/// catching cases where the server sends data without `\n\n` terminators.
const MAX_SSE_EVENT_BYTES: usize = 50 * 1024 * 1024;

pub struct HttpMcpClient {
    /// Display name of the MCP server, used for logging
    server_name: String,
    /// HTTP client with overall timeout for regular requests
    client: Client,
    /// HTTP client without overall timeout for SSE streams (sampling)
    stream_client: Client,
    base_url: String,
    connected: bool,
    session_id: Arc<RwLock<Option<String>>>,
    sampling_handler: Option<Arc<dyn SamplingHandler>>,
}

impl HttpMcpClient {
    pub fn new(server: McpServer) -> Result<Self, AppError> {
        Self::new_internal(server, None)
    }

    pub fn new_with_sampling(
        server: McpServer,
        handler: Arc<dyn SamplingHandler>,
    ) -> Result<Self, AppError> {
        Self::new_internal(server, Some(handler))
    }

    fn new_internal(
        server: McpServer,
        sampling_handler: Option<Arc<dyn SamplingHandler>>,
    ) -> Result<Self, AppError> {
        if server.transport_type != TransportType::Http {
            return Err(AppError::bad_request("INVALID_TRANSPORT", "Only HTTP transport supported"));
        }

        let base_url = server.url.clone()
            .ok_or_else(|| AppError::bad_request("MISSING_URL", "Missing URL for HTTP transport"))?;

        let mut headers = reqwest::header::HeaderMap::new();
        if let Some(headers_map) = server.headers.as_object() {
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
        }

        let timeout_secs = server.timeout_seconds.max(1) as u64;

        // Regular client has an overall timeout
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .default_headers(headers.clone())
            .build()
            .map_err(|e| AppError::internal_error(format!("Failed to create HTTP client: {}", e)))?;

        // Streaming client: only connect timeout (no overall timeout — SSE streams can be long)
        let stream_client = Client::builder()
            .connect_timeout(Duration::from_secs(timeout_secs))
            .default_headers(headers)
            .build()
            .map_err(|e| AppError::internal_error(format!("Failed to create stream client: {}", e)))?;

        Ok(Self {
            server_name: server.name.clone(),
            client,
            stream_client,
            base_url,
            connected: false,
            session_id: Arc::new(RwLock::new(None)),
            sampling_handler,
        })
    }

    fn get_session_id(&self) -> Option<String> {
        match self.session_id.read() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => {
                tracing::error!("[mcp] session_id RwLock poisoned — recovering");
                poisoned.into_inner().clone()
            }
        }
    }

    fn set_session_id(&self, id: &str) {
        match self.session_id.write() {
            Ok(mut guard) => *guard = Some(id.to_string()),
            Err(poisoned) => {
                tracing::error!("[mcp] session_id RwLock poisoned — recovering");
                *poisoned.into_inner() = Some(id.to_string());
            }
        }
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

        // Use the URL as-is (user provides full endpoint URL including path)
        let url = self.base_url.clone();

        let mut request = self.client
            .post(&url)
            .header("Accept", "application/json")
            .json(&request_body);

        // Add session ID if available
        if let Some(session_id) = self.get_session_id() {
            request = request.header("mcp-session-id", session_id);
        }

        let response = request.send()
            .await
            .map_err(|e| AppError::internal_error(format!("HTTP request failed: {}", e)))?;

        let status = response.status();

        // Extract session ID from response headers if present
        if let Some(session_id) = response.headers().get("mcp-session-id") {
            if let Ok(session_str) = session_id.to_str() {
                self.set_session_id(session_str);
            }
        }

        // Get response text
        let response_text = response.text().await
            .map_err(|e| AppError::internal_error(format!("Failed to get response text: {}", e)))?;

        // Check HTTP status before attempting JSON parse
        if !status.is_success() {
            return Err(AppError::internal_error(format!(
                "MCP server returned HTTP {}: {}",
                status,
                response_text.chars().take(200).collect::<String>()
            )));
        }

        // Trim body to handle responses with leading/trailing whitespace
        let trimmed = response_text.trim();

        // Parse SSE format: extract JSON from "data: {...}" lines
        let response_json: Value = if trimmed.contains("data: ") {
            // SSE format - extract first data line
            let mut found_data = None;
            for line in trimmed.lines() {
                if let Some(data) = line.strip_prefix("data: ") {
                    found_data = Some(serde_json::from_str(data)
                        .map_err(|e| AppError::internal_error(format!("Failed to parse SSE data: {}", e)))?);
                    break;
                }
            }
            found_data.ok_or_else(|| AppError::internal_error("No data found in SSE response"))?
        } else {
            // Plain JSON format
            serde_json::from_str(trimmed)
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

    /// Call a tool with SSE streaming + inline sampling/elicitation support.
    ///
    /// Runs in a completely independent `tokio::spawn` task (see `call_tool`) so that
    /// `req.send().await` is not subject to cancellation from the Axum SSE handler task
    /// that drives the user's chat stream.
    async fn call_tool_with_sampling(
        handler: Arc<dyn SamplingHandler>,
        stream_client: Client,
        url: String,
        session_id_arc: Arc<RwLock<Option<String>>>,
        server_name: String,
        name: String,
        arguments: Value,
        message_id: Option<uuid::Uuid>,
        sse_tx: Option<tokio::sync::mpsc::UnboundedSender<Result<axum::response::sse::Event, std::convert::Infallible>>>,
        elicit_notify_tx: Option<tokio::sync::mpsc::UnboundedSender<crate::modules::mcp::elicitation::models::ElicitationStartedNotification>>,
    ) -> Result<ToolResult, AppError> {
        use crate::modules::mcp::sampling::models::{
            SamplingContent, SamplingCreateMessageRequest, SamplingCreateMessageResult,
        };

        // Local helpers that operate on the Arc'd session_id
        let get_sid = {
            let arc = session_id_arc.clone();
            move || match arc.read() {
                Ok(guard) => guard.clone(),
                Err(poisoned) => {
                    tracing::error!("[mcp] session_id RwLock poisoned — recovering");
                    poisoned.into_inner().clone()
                }
            }
        };
        let set_sid = {
            let arc = session_id_arc.clone();
            move |id: &str| match arc.write() {
                Ok(mut guard) => *guard = Some(id.to_string()),
                Err(poisoned) => {
                    tracing::error!("[mcp] session_id RwLock poisoned — recovering");
                    *poisoned.into_inner() = Some(id.to_string());
                }
            }
        };

        tracing::info!(
            "[sampling] call_tool_with_sampling: server='{}' tool='{}'",
            server_name, name,
        );

        let request_body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": name,
                "arguments": arguments
            }
        });

        let mut req = stream_client
            .post(&url)
            .header("Accept", "text/event-stream")
            .header("Content-Type", "application/json")
            .json(&request_body);

        let sid = get_sid();
        if let Some(ref s) = sid {
            req = req.header("mcp-session-id", s.as_str());
        }

        tracing::info!(
            "[sampling] tools/call → url={} headers={{Accept: text/event-stream, Content-Type: application/json, mcp-session-id: {:?}}} body={}",
            url, sid, request_body
        );

        tracing::info!("[sampling] sending tools/call SSE request");

        let response = req.send().await
            .map_err(|e| {
                tracing::error!("[sampling] SSE request failed: {}", e);
                AppError::internal_error(format!("SSE request failed: {}", e))
            })?;

        tracing::info!("[sampling] SSE response headers received");

        if let Some(sid) = response.headers().get("mcp-session-id") {
            if let Ok(s) = sid.to_str() {
                set_sid(s);
            }
        }

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::internal_error(format!("MCP HTTP error {}: {}", status, error_text)));
        }

        tracing::info!(
            "[sampling] SSE stream open: status={} content-type={:?}",
            status,
            response.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("none"),
        );

        let mut byte_stream = response.bytes_stream();
        let mut buffer = String::new();

        tracing::info!("[sampling] Entering SSE byte-stream loop");

        loop {
            match byte_stream.next().await {
                Some(Ok(chunk)) => {
                    let chunk_str = String::from_utf8_lossy(&chunk);
                    tracing::debug!(
                        "[sampling] SSE chunk: {} bytes",
                        chunk.len(),
                    );

                    // Guard against unbounded buffer growth from a server that never sends \n\n
                    if buffer.len() + chunk.len() > MAX_SSE_EVENT_BYTES {
                        return Err(AppError::internal_error(
                            "MCP SSE event exceeded 50MB limit — server may be sending malformed events without \\n\\n terminator"
                        ));
                    }
                    buffer.push_str(&chunk_str);

                    // Process complete SSE events (separated by double newline)
                    while let Some(event_end) = buffer.find("\n\n") {
                        let event_block = buffer[..event_end].to_string();
                        buffer.drain(..event_end + 2);

                        // Extract data line from event block
                        let data_line = event_block.lines()
                            .find(|l| l.starts_with("data: "))
                            .map(|l| &l[6..]);

                        let data = match data_line {
                            Some(d) => d,
                            None => continue,
                        };

                        let json: Value = match serde_json::from_str(data) {
                            Ok(v) => v,
                            Err(e) => {
                                tracing::warn!("Failed to parse MCP SSE event: {} — data: {}", e, &data[..data.len().min(200)]);
                                continue;
                            }
                        };

                        // Check if this is a server→client request (elicitation or sampling)
                        if let Some(method) = json.get("method").and_then(|m| m.as_str()) {
                            // --- Elicitation (MCP spec 2025-03-26+) ---
                            // The MCP server needs structured human input; pause the loop and wait.
                            if method == "elicitation/create" {
                                let req_id = json.get("id").cloned().unwrap_or(Value::Null);
                                let params = json.get("params").cloned().unwrap_or(Value::Null);
                                let message = params.get("message").and_then(|m| m.as_str()).unwrap_or("").to_string();
                                let requested_schema = params.get("requestedSchema").cloned().unwrap_or(Value::Null);

                                tracing::info!(
                                    "[elicitation] received elicitation/create id={:?} from '{}'",
                                    req_id, server_name
                                );

                                // Generate a fresh per-elicitation UUID as the registry key.
                                // Using a random UUID (not message_id) lets sequential elicitations
                                // within the same tool call each get their own unique key.
                                let elicitation_id = uuid::Uuid::new_v4();
                                // Pre-generate the content_id for the DB row (written by the extension layer)
                                let content_id = uuid::Uuid::new_v4();

                                // Register a oneshot channel in the global registry keyed by elicitation_id
                                let (elicit_tx, elicit_rx) = tokio::sync::oneshot::channel::<crate::modules::mcp::elicitation::models::ElicitationResponse>();
                                crate::modules::mcp::elicitation::registry::register(elicitation_id, elicit_tx, Some(content_id));

                                // Notify the extension layer (mcp.rs) so it can persist the content block via Repos.
                                // http.rs has no DB access — the notification channel bridges to the higher layer.
                                if let Some(ref notify_tx) = elicit_notify_tx {
                                    let _ = notify_tx.send(crate::modules::mcp::elicitation::models::ElicitationStartedNotification {
                                        elicitation_id,
                                        content_id,
                                        message_id,
                                        message: message.clone(),
                                        requested_schema: requested_schema.clone(),
                                        server: server_name.clone(),
                                    });
                                }

                                // Send SSE event to browser (raw JSON — no import from chat/)
                                if let Some(ref tx) = sse_tx {
                                    let event_data = serde_json::json!({
                                        "elicitation_id": elicitation_id.to_string(),
                                        "message_id": message_id.map(|m| m.to_string()),
                                        "message": message,
                                        "requested_schema": requested_schema,
                                        "server": server_name,
                                    });
                                    let event = axum::response::sse::Event::default()
                                        .event("mcpElicitationRequired")
                                        .data(event_data.to_string());
                                    if tx.send(Ok(event)).is_err() {
                                        tracing::warn!("[elicitation] SSE channel closed — sending cancel");
                                        let _ = crate::modules::mcp::elicitation::registry::remove(elicitation_id);
                                        // Post cancel to unblock the MCP server
                                        let body = serde_json::json!({
                                            "jsonrpc": "2.0",
                                            "id": req_id,
                                            "result": { "action": "cancel" }
                                        });
                                        let mut post = stream_client.post(&url).json(&body);
                                        if let Some(s) = get_sid() {
                                            post = post.header("mcp-session-id", s);
                                        }
                                        let _ = post.send().await;
                                        continue;
                                    }
                                } else {
                                    // No SSE channel — immediately cancel
                                    tracing::warn!("[elicitation] no sse_tx available — sending cancel for id={:?}", req_id);
                                    let _ = crate::modules::mcp::elicitation::registry::remove(elicitation_id);
                                    let body = serde_json::json!({
                                        "jsonrpc": "2.0",
                                        "id": req_id,
                                        "result": { "action": "cancel" }
                                    });
                                    let mut post = stream_client.post(&url).json(&body);
                                    if let Some(s) = get_sid() {
                                        post = post.header("mcp-session-id", s);
                                    }
                                    let _ = post.send().await;
                                    continue;
                                }

                                // Block the loop until the user responds.
                                // No timeout — the MCP spec defines none and users need time to think.
                                // Cleanup happens via SSE close: when sse_tx.send() fails,
                                // registry::remove() is called which drops the tx, causing elicit_rx
                                // to return Err(RecvError) below.
                                let user_response = match elicit_rx.await {
                                    Ok(response) => response,
                                    Err(_) => {
                                        // Channel dropped — SSE closed or registry removed
                                        tracing::warn!("[elicitation] oneshot channel dropped for id={:?}", req_id);
                                        crate::modules::mcp::elicitation::models::ElicitationResponse {
                                            action: "cancel".to_string(),
                                            content: None,
                                        }
                                    }
                                };

                                // Post the user's response back to the MCP server
                                let result_value = if user_response.action == "accept" {
                                    serde_json::json!({
                                        "action": user_response.action,
                                        "content": user_response.content.unwrap_or(Value::Null),
                                    })
                                } else {
                                    serde_json::json!({ "action": user_response.action })
                                };
                                let body = serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "id": req_id,
                                    "result": result_value
                                });
                                let mut post = stream_client.post(&url).json(&body);
                                if let Some(s) = get_sid() {
                                    post = post.header("mcp-session-id", s);
                                }
                                match post.send().await {
                                    Ok(r) => tracing::info!(
                                        "[elicitation] POSTed response id={:?} action='{}' → HTTP {}",
                                        req_id, &result_value.get("action").and_then(|v| v.as_str()).unwrap_or("?"), r.status()
                                    ),
                                    Err(e) => tracing::error!(
                                        "[elicitation] Failed to POST response id={:?}: {}",
                                        req_id, e
                                    ),
                                }

                                continue; // back to byte_stream.next().await
                            }

                            if method == "sampling/createMessage" {
                                let req_id = json.get("id").cloned().unwrap_or(Value::Null);
                                tracing::info!(
                                    "[sampling] received sampling/createMessage id={:?} from '{}'",
                                    req_id, server_name
                                );
                                let params = json.get("params").cloned().unwrap_or(Value::Null);

                                // Spawn the LLM call + POST in a separate task so the SSE
                                // reading loop can continue being polled by the executor.
                                // Clone into the spawn block using block-before-spawn to avoid
                                // moving captured variables out of the loop.
                                match serde_json::from_value::<SamplingCreateMessageRequest>(params) {
                                    Ok(sampling_req) => {
                                        tokio::spawn({
                                            let handler = handler.clone();
                                            let client = stream_client.clone();
                                            let url = url.clone();
                                            let sid = get_sid();
                                            let req_id = req_id.clone();
                                            async move {
                                                let result = match handler.create_message(sampling_req).await {
                                                    Ok(r) => r,
                                                    Err(e) => {
                                                        tracing::error!(
                                                            "[sampling] handler error id={:?}: {}",
                                                            req_id, e
                                                        );
                                                        SamplingCreateMessageResult {
                                                            role: "assistant".to_string(),
                                                            content: SamplingContent::Text {
                                                                text: format!("Error: {}", e),
                                                            },
                                                            model: "unknown".to_string(),
                                                            stop_reason: Some("error".to_string()),
                                                        }
                                                    }
                                                };

                                                let body = serde_json::json!({
                                                    "jsonrpc": "2.0",
                                                    "id": req_id,
                                                    "result": result
                                                });
                                                let mut post = client.post(&url).json(&body);
                                                if let Some(s) = sid {
                                                    post = post.header("mcp-session-id", s);
                                                }
                                                match post.send().await {
                                                    Ok(r) => tracing::info!(
                                                        "[sampling] POSTed sampling response id={:?} → HTTP {}",
                                                        req_id, r.status()
                                                    ),
                                                    Err(e) => tracing::error!(
                                                        "[sampling] Failed to POST sampling response id={:?}: {}",
                                                        req_id, e
                                                    ),
                                                }
                                            }
                                        });
                                    }
                                    Err(e) => {
                                        tracing::error!(
                                            "[sampling] Failed to parse sampling request id={:?}: {}",
                                            req_id, e
                                        );
                                        // POST an error result so the MCP server is unblocked
                                        tokio::spawn({
                                            let client = stream_client.clone();
                                            let url = url.clone();
                                            let sid = get_sid();
                                            let req_id = req_id.clone();
                                            async move {
                                                let error_result = SamplingCreateMessageResult {
                                                    role: "assistant".to_string(),
                                                    content: SamplingContent::Text {
                                                        text: format!("Error: failed to parse sampling request: {}", e),
                                                    },
                                                    model: "unknown".to_string(),
                                                    stop_reason: Some("error".to_string()),
                                                };
                                                let body = serde_json::json!({
                                                    "jsonrpc": "2.0",
                                                    "id": req_id,
                                                    "result": error_result
                                                });
                                                let mut post = client.post(&url).json(&body);
                                                if let Some(s) = sid {
                                                    post = post.header("mcp-session-id", s);
                                                }
                                                if let Err(post_err) = post.send().await {
                                                    tracing::error!(
                                                        "[sampling] Failed to POST parse-error response id={:?}: {}",
                                                        req_id, post_err
                                                    );
                                                }
                                            }
                                        });
                                    }
                                }

                                continue; // back to byte_stream.next().await
                            }
                        }

                        // Check if this is the final tool result
                        if json.get("result").is_some() {
                            let result_value = json["result"].clone();
                            return serde_json::from_value(result_value)
                                .map_err(|e| AppError::internal_error(format!("Failed to parse tool result: {}", e)));
                        }

                        if let Some(error) = json.get("error") {
                            return Err(AppError::internal_error(format!("MCP tool error: {}", error)));
                        }
                    }
                }
                Some(Err(e)) => {
                    return Err(AppError::internal_error(format!("SSE stream error: {}", e)));
                }
                None => {
                    return Err(AppError::internal_error("SSE stream ended without tool result"));
                }
            }
        }
    }

    /// Call a tool on an elicitation-capable server (no sampling handler).
    ///
    /// Receives an already-sent `reqwest::Response` from `call_tool` (Content-Type was
    /// verified as `text/event-stream` before this is called).  Runs the same SSE
    /// byte-stream loop as `call_tool_with_sampling` but without a sampling handler:
    /// - `elicitation/create` events are fully handled (identical to sampling path)
    /// - `sampling/createMessage` events are rejected with a JSON-RPC error
    async fn call_tool_with_elicitation(
        response: reqwest::Response,
        stream_client: Client,
        url: String,
        session_id_arc: Arc<RwLock<Option<String>>>,
        server_name: String,
        message_id: Option<uuid::Uuid>,
        sse_tx: Option<tokio::sync::mpsc::UnboundedSender<Result<axum::response::sse::Event, std::convert::Infallible>>>,
        elicit_notify_tx: Option<tokio::sync::mpsc::UnboundedSender<crate::modules::mcp::elicitation::models::ElicitationStartedNotification>>,
    ) -> Result<ToolResult, AppError> {
        let get_sid = {
            let arc = session_id_arc.clone();
            move || match arc.read() {
                Ok(guard) => guard.clone(),
                Err(poisoned) => {
                    tracing::error!("[mcp] session_id RwLock poisoned — recovering");
                    poisoned.into_inner().clone()
                }
            }
        };

        tracing::info!(
            "[elicitation] call_tool_with_elicitation: server='{}'",
            server_name,
        );

        let mut byte_stream = response.bytes_stream();
        let mut buffer = String::new();

        loop {
            match byte_stream.next().await {
                Some(Ok(chunk)) => {
                    tracing::info!("[elicitation] received SSE chunk: {} bytes from '{}'", chunk.len(), server_name);

                    if buffer.len() + chunk.len() > MAX_SSE_EVENT_BYTES {
                        return Err(AppError::internal_error(
                            "MCP SSE event exceeded 50MB limit — server may be sending malformed events without \\n\\n terminator"
                        ));
                    }
                    buffer.push_str(&String::from_utf8_lossy(&chunk));

                    // Support both LF-only (\n\n) and CRLF (\r\n\r\n) event separators per SSE spec
                    let sep = if buffer.contains("\r\n\r\n") { "\r\n\r\n" } else { "\n\n" };
                    while let Some(event_end) = buffer.find(sep) {
                        let event_block = buffer[..event_end].to_string();
                        buffer.drain(..event_end + sep.len());

                        let data_line = event_block.lines()
                            .find(|l| l.starts_with("data: "))
                            .map(|l| &l[6..]);

                        let data = match data_line {
                            Some(d) => d,
                            None => continue,
                        };

                        let json: Value = match serde_json::from_str(data) {
                            Ok(v) => v,
                            Err(e) => {
                                tracing::warn!("[elicitation] Failed to parse SSE event: {} — data: {}", e, &data[..data.len().min(200)]);
                                continue;
                            }
                        };

                        tracing::info!(
                            "[elicitation] parsed SSE event from '{}': method={:?} has_result={} has_error={}",
                            server_name,
                            json.get("method").and_then(|m| m.as_str()),
                            json.get("result").is_some(),
                            json.get("error").is_some(),
                        );

                        if let Some(method) = json.get("method").and_then(|m| m.as_str()) {
                            // --- Elicitation (identical to call_tool_with_sampling) ---
                            if method == "elicitation/create" {
                                let req_id = json.get("id").cloned().unwrap_or(Value::Null);
                                let params = json.get("params").cloned().unwrap_or(Value::Null);
                                let message = params.get("message").and_then(|m| m.as_str()).unwrap_or("").to_string();
                                let requested_schema = params.get("requestedSchema").cloned().unwrap_or(Value::Null);

                                tracing::info!(
                                    "[elicitation] received elicitation/create id={:?} from '{}'",
                                    req_id, server_name
                                );

                                let elicitation_id = uuid::Uuid::new_v4();
                                let content_id = uuid::Uuid::new_v4();

                                let (elicit_tx, elicit_rx) = tokio::sync::oneshot::channel::<crate::modules::mcp::elicitation::models::ElicitationResponse>();
                                crate::modules::mcp::elicitation::registry::register(elicitation_id, elicit_tx, Some(content_id));

                                // Notify the extension layer (mcp.rs) so it can persist the content block via Repos
                                if let Some(ref notify_tx) = elicit_notify_tx {
                                    let _ = notify_tx.send(crate::modules::mcp::elicitation::models::ElicitationStartedNotification {
                                        elicitation_id,
                                        content_id,
                                        message_id,
                                        message: message.clone(),
                                        requested_schema: requested_schema.clone(),
                                        server: server_name.clone(),
                                    });
                                }

                                if let Some(ref tx) = sse_tx {
                                    let event_data = serde_json::json!({
                                        "elicitation_id": elicitation_id.to_string(),
                                        "message_id": message_id.map(|m| m.to_string()),
                                        "message": message,
                                        "requested_schema": requested_schema,
                                        "server": server_name,
                                    });
                                    let event = axum::response::sse::Event::default()
                                        .event("mcpElicitationRequired")
                                        .data(event_data.to_string());
                                    if tx.send(Ok(event)).is_err() {
                                        tracing::warn!("[elicitation] SSE channel closed — sending cancel");
                                        let _ = crate::modules::mcp::elicitation::registry::remove(elicitation_id);
                                        let body = serde_json::json!({
                                            "jsonrpc": "2.0",
                                            "id": req_id,
                                            "result": { "action": "cancel" }
                                        });
                                        let mut post = stream_client.post(&url).json(&body);
                                        if let Some(s) = get_sid() {
                                            post = post.header("mcp-session-id", s);
                                        }
                                        let _ = post.send().await;
                                        continue;
                                    }
                                } else {
                                    tracing::warn!("[elicitation] no sse_tx available — sending cancel for id={:?}", req_id);
                                    let _ = crate::modules::mcp::elicitation::registry::remove(elicitation_id);
                                    let body = serde_json::json!({
                                        "jsonrpc": "2.0",
                                        "id": req_id,
                                        "result": { "action": "cancel" }
                                    });
                                    let mut post = stream_client.post(&url).json(&body);
                                    if let Some(s) = get_sid() {
                                        post = post.header("mcp-session-id", s);
                                    }
                                    let _ = post.send().await;
                                    continue;
                                }

                                let user_response = match elicit_rx.await {
                                    Ok(response) => response,
                                    Err(_) => {
                                        tracing::warn!("[elicitation] oneshot channel dropped for id={:?}", req_id);
                                        crate::modules::mcp::elicitation::models::ElicitationResponse {
                                            action: "cancel".to_string(),
                                            content: None,
                                        }
                                    }
                                };

                                let result_value = if user_response.action == "accept" {
                                    serde_json::json!({
                                        "action": user_response.action,
                                        "content": user_response.content.unwrap_or(Value::Null),
                                    })
                                } else {
                                    serde_json::json!({ "action": user_response.action })
                                };
                                let body = serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "id": req_id,
                                    "result": result_value
                                });
                                let mut post = stream_client.post(&url).json(&body);
                                if let Some(s) = get_sid() {
                                    post = post.header("mcp-session-id", s);
                                }
                                match post.send().await {
                                    Ok(r) => tracing::info!(
                                        "[elicitation] POSTed response id={:?} action='{}' → HTTP {}",
                                        req_id,
                                        &result_value.get("action").and_then(|v| v.as_str()).unwrap_or("?"),
                                        r.status()
                                    ),
                                    Err(e) => tracing::error!(
                                        "[elicitation] Failed to POST response id={:?}: {}",
                                        req_id, e
                                    ),
                                }

                                continue;
                            }

                            // --- sampling/createMessage is not supported on this path ---
                            if method == "sampling/createMessage" {
                                let req_id = json.get("id").cloned().unwrap_or(Value::Null);
                                tracing::warn!(
                                    "[elicitation] server '{}' sent sampling/createMessage but sampling is not enabled on this session; rejecting",
                                    server_name
                                );
                                let body = serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "id": req_id,
                                    "error": {
                                        "code": -32601,
                                        "message": "sampling/createMessage is not supported — enable sampling on this MCP server to use this feature"
                                    }
                                });
                                let mut post = stream_client.post(&url).json(&body);
                                if let Some(s) = get_sid() {
                                    post = post.header("mcp-session-id", s);
                                }
                                let _ = post.send().await;
                                continue;
                            }
                        }

                        if json.get("result").is_some() {
                            let result_value = json["result"].clone();
                            return serde_json::from_value(result_value)
                                .map_err(|e| AppError::internal_error(format!("Failed to parse tool result: {}", e)));
                        }

                        if let Some(error) = json.get("error") {
                            return Err(AppError::internal_error(format!("MCP tool error: {}", error)));
                        }
                    }
                }
                Some(Err(e)) => {
                    return Err(AppError::internal_error(format!("SSE stream error: {}", e)));
                }
                None => {
                    return Err(AppError::internal_error("SSE stream ended without tool result"));
                }
            }
        }
    }
}

#[async_trait]
impl McpClient for HttpMcpClient {
    async fn connect(&mut self) -> Result<(), AppError> {
        // Advertise capabilities: always include elicitation; add sampling if handler present
        let capabilities = if self.sampling_handler.is_some() {
            serde_json::json!({ "sampling": {}, "elicitation": {} })
        } else {
            serde_json::json!({ "elicitation": {} })
        };

        let _: Value = self.request("initialize", serde_json::json!({
            "protocolVersion": "2025-03-26",
            "capabilities": capabilities,
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
        message_id: Option<uuid::Uuid>,
        sse_tx: Option<tokio::sync::mpsc::UnboundedSender<Result<axum::response::sse::Event, std::convert::Infallible>>>,
        elicit_notify_tx: Option<tokio::sync::mpsc::UnboundedSender<crate::modules::mcp::elicitation::models::ElicitationStartedNotification>>,
    ) -> Result<ToolResult, AppError> {
        if !self.is_connected() {
            return Err(AppError::internal_error("Not connected"));
        }

        // Use sampling-aware SSE streaming if a sampling handler is present.
        // Spawn in a completely independent task so that req.send().await inside
        // call_tool_with_sampling is not subject to cancellation from the
        // Axum SSE handler task that drives the user's chat stream.
        if let Some(handler) = self.sampling_handler.clone() {
            let stream_client  = self.stream_client.clone();
            let url            = self.base_url.clone();
            let session_id_arc = self.session_id.clone();
            let server_name    = self.server_name.clone();
            let name_owned     = name.to_string();
            let arguments_owned = arguments;

            let (result_tx, result_rx) =
                tokio::sync::oneshot::channel::<Result<ToolResult, AppError>>();

            tokio::spawn(async move {
                let result = HttpMcpClient::call_tool_with_sampling(
                    handler,
                    stream_client,
                    url,
                    session_id_arc,
                    server_name,
                    name_owned,
                    arguments_owned,
                    message_id,
                    sse_tx,
                    elicit_notify_tx,
                )
                .await;
                let _ = result_tx.send(result);
            });

            return result_rx
                .await
                .map_err(|_| AppError::internal_error("Sampling task was cancelled"))?;
        }

        // Non-sampling call: send one request with Accept: text/event-stream, then route
        // on the response Content-Type.  Plain-JSON servers return application/json and are
        // parsed directly (Branch 3); elicitation-capable servers return text/event-stream
        // and are handed to call_tool_with_elicitation (Branch 2).
        // Spawned in an independent task for the same cancellation-safety reason as Branch 1.
        let stream_client        = self.stream_client.clone();
        let url                  = self.base_url.clone();
        let session_id_arc       = self.session_id.clone();
        let server_name          = self.server_name.clone();
        let name_owned           = name.to_string();
        let arguments_owned      = arguments;
        let message_id_owned     = message_id;
        let elicit_notify_owned  = elicit_notify_tx;

        let (result_tx, result_rx) =
            tokio::sync::oneshot::channel::<Result<ToolResult, AppError>>();

        tokio::spawn(async move {
            // get_sid / set_sid — same pattern as call_tool_with_sampling
            let get_sid = {
                let arc = session_id_arc.clone();
                move || match arc.read() {
                    Ok(guard) => guard.clone(),
                    Err(poisoned) => {
                        tracing::error!("[mcp] session_id RwLock poisoned — recovering");
                        poisoned.into_inner().clone()
                    }
                }
            };
            let set_sid = {
                let arc = session_id_arc.clone();
                move |id: &str| match arc.write() {
                    Ok(mut guard) => *guard = Some(id.to_string()),
                    Err(poisoned) => {
                        tracing::error!("[mcp] session_id RwLock poisoned — recovering");
                        *poisoned.into_inner() = Some(id.to_string());
                    }
                }
            };

            let request_body = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/call",
                "params": {
                    "name": name_owned,
                    "arguments": arguments_owned
                }
            });

            let mut req = stream_client
                .post(&url)
                .header("Accept", "text/event-stream")
                .header("Content-Type", "application/json")
                .json(&request_body);

            if let Some(s) = get_sid() {
                req = req.header("mcp-session-id", s);
            }

            let response = match req.send().await {
                Ok(r) => r,
                Err(e) => {
                    let _ = result_tx.send(Err(AppError::internal_error(format!("MCP request failed: {}", e))));
                    return;
                }
            };

            if let Some(sid) = response.headers().get("mcp-session-id") {
                if let Ok(s) = sid.to_str() {
                    set_sid(s);
                }
            }

            let status = response.status();
            if !status.is_success() {
                let error_text = response.text().await.unwrap_or_default();
                let _ = result_tx.send(Err(AppError::internal_error(format!("MCP HTTP error {}: {}", status, error_text))));
                return;
            }

            let content_type = response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();

            let result = if content_type.contains("text/event-stream") {
                // Branch 2: server speaks SSE — may send elicitation/create mid-stream
                HttpMcpClient::call_tool_with_elicitation(
                    response,
                    stream_client,
                    url,
                    session_id_arc,
                    server_name,
                    message_id_owned,
                    sse_tx,
                    elicit_notify_owned,
                )
                .await
            } else {
                // Branch 3: plain JSON — parse body the same way self.request() does
                let text = match response.text().await {
                    Ok(t) => t,
                    Err(e) => {
                        return { let _ = result_tx.send(Err(AppError::internal_error(format!("Failed to read response: {}", e)))); }
                    }
                };
                let trimmed = text.trim();
                let json: serde_json::Value = if trimmed.contains("data: ") {
                    let mut found = None;
                    for line in trimmed.lines() {
                        if let Some(data) = line.strip_prefix("data: ") {
                            found = Some(match serde_json::from_str(data) {
                                Ok(v) => v,
                                Err(e) => {
                                    let _ = result_tx.send(Err(AppError::internal_error(format!("Failed to parse SSE data: {}", e))));
                                    return;
                                }
                            });
                            break;
                        }
                    }
                    match found {
                        Some(v) => v,
                        None => {
                            let _ = result_tx.send(Err(AppError::internal_error("No data found in SSE response")));
                            return;
                        }
                    }
                } else {
                    match serde_json::from_str(trimmed) {
                        Ok(v) => v,
                        Err(e) => {
                            let _ = result_tx.send(Err(AppError::internal_error(format!("Failed to parse response: {}", e))));
                            return;
                        }
                    }
                };
                if let Some(error) = json.get("error") {
                    return { let _ = result_tx.send(Err(AppError::internal_error(format!("MCP error: {}", error)))); }
                }
                match json.get("result") {
                    Some(result_val) => serde_json::from_value(result_val.clone())
                        .map_err(|e| AppError::internal_error(format!("Failed to deserialize result: {}", e))),
                    None => Err(AppError::internal_error("Missing result in response")),
                }
            };

            let _ = result_tx.send(result);
        });

        result_rx
            .await
            .map_err(|_| AppError::internal_error("Tool call task was cancelled"))?
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

// Tests for this module live in tests/mcp/mod.rs (see test_call_tool_with_sampling_sse_roundtrip
// and test_call_tool_with_real_llm_sampling).
