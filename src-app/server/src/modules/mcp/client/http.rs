use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::Value;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use super::traits::{McpClient, Prompt, PromptResult, Resource, Tool, ToolResult};
use crate::common::AppError;
use crate::modules::mcp::models::{McpServer, TransportType};
use crate::modules::mcp::sampling::SamplingHandler;

/// Maximum bytes for a single buffered SSE event from the MCP server.
/// 50 MB is generous enough for long prompt-enhancement payloads while still
/// catching cases where the server sends data without `\n\n` terminators.
const MAX_SSE_EVENT_BYTES: usize = 50 * 1024 * 1024;

/// MCP protocol version this client implements. Sent in `initialize` and on
/// every subsequent request via the `MCP-Protocol-Version` header. Bumped
/// whenever we audit + verify against a newer spec — current is 2025-11-25.
const MCP_PROTOCOL_VERSION: &str = "2025-11-25";

/// Parse a Server-Sent Events response body and return the JSON-RPC message
/// whose `id` matches the requested id. Drops notifications and unrelated
/// requests/responses (logs them at debug level). Per MCP spec § Transports:
/// "The server MAY send JSON-RPC requests and notifications before sending
/// the JSON-RPC response."
fn extract_response_by_id(sse_body: &str, expected_id: i64) -> Result<Value, AppError> {
    // SSE event separator may be \n\n or \r\n\r\n per the SSE spec.
    let body = sse_body.replace("\r\n", "\n");
    for event_block in body.split("\n\n") {
        // Each event consists of lines like "field: value". We only care
        // about the `data:` lines; multiple data lines in one event are
        // concatenated with newlines per the SSE spec.
        let mut data = String::new();
        for line in event_block.lines() {
            if let Some(rest) = line.strip_prefix("data: ") {
                if !data.is_empty() { data.push('\n'); }
                data.push_str(rest);
            } else if let Some(rest) = line.strip_prefix("data:") {
                // SSE allows "data:" with no space
                if !data.is_empty() { data.push('\n'); }
                data.push_str(rest);
            }
        }
        if data.is_empty() { continue; }

        let json: Value = match serde_json::from_str(&data) {
            Ok(v) => v,
            Err(e) => {
                tracing::debug!("[mcp] SSE data line was not valid JSON: {} ({})", e, &data[..data.len().min(80)]);
                continue;
            }
        };

        // If this message carries our id (either result or error response),
        // it's the one we're waiting for.
        let id_matches = json.get("id")
            .and_then(|v| v.as_i64())
            .map(|i| i == expected_id)
            .unwrap_or(false);

        if id_matches {
            return Ok(json);
        }

        // Otherwise log and continue — could be a server-initiated request
        // (sampling/elicitation handled elsewhere), a progress notification, etc.
        tracing::debug!(
            "[mcp] SSE event before response: method={:?} id={:?}",
            json.get("method").and_then(|m| m.as_str()),
            json.get("id"),
        );
    }
    Err(AppError::internal_error(format!(
        "SSE stream ended without a response for request id={}", expected_id
    )))
}

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
    /// Monotonic JSON-RPC request id counter. Per MCP spec: "The request ID
    /// MUST NOT have been previously used by the requestor within the same
    /// session." Atomic so it can be cloned into background tasks.
    next_request_id: Arc<AtomicI64>,
    /// Protocol version negotiated during `initialize`. Sent on subsequent
    /// requests via the `MCP-Protocol-Version` header (MCP spec § Transports).
    /// `None` until initialize completes.
    negotiated_protocol_version: Arc<RwLock<Option<String>>>,
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
            next_request_id: Arc::new(AtomicI64::new(1)),
            negotiated_protocol_version: Arc::new(RwLock::new(None)),
            sampling_handler,
        })
    }

    /// Allocate the next monotonically increasing JSON-RPC request id.
    fn next_id(&self) -> i64 {
        self.next_request_id.fetch_add(1, Ordering::Relaxed)
    }

    fn get_protocol_version(&self) -> Option<String> {
        match self.negotiated_protocol_version.read() {
            Ok(g) => g.clone(),
            Err(p) => {
                tracing::error!("[mcp] protocol_version RwLock poisoned — recovering");
                p.into_inner().clone()
            }
        }
    }

    fn set_protocol_version(&self, v: &str) {
        match self.negotiated_protocol_version.write() {
            Ok(mut g) => *g = Some(v.to_string()),
            Err(p) => {
                tracing::error!("[mcp] protocol_version RwLock poisoned — recovering");
                *p.into_inner() = Some(v.to_string());
            }
        }
    }

    /// Send a JSON-RPC notification (no `id`, no response expected).
    /// Per MCP spec § Transports: server MUST respond with HTTP 202 Accepted
    /// and no body when accepting a notification.
    /// Per JSON-RPC 2.0: `params` MUST be omitted entirely when not used
    /// (sending `"params": null` is a parse error — strict servers reject it).
    async fn send_notification(&self, method: &str, params: Value) -> Result<(), AppError> {
        let body = if params.is_null() {
            serde_json::json!({
                "jsonrpc": "2.0",
                "method": method,
            })
        } else {
            serde_json::json!({
                "jsonrpc": "2.0",
                "method": method,
                "params": params,
            })
        };

        let mut req = self.client
            .post(&self.base_url)
            .header("Accept", "application/json, text/event-stream")
            .json(&body);
        if let Some(sid) = self.get_session_id() {
            req = req.header("mcp-session-id", sid);
        }
        if let Some(ver) = self.get_protocol_version() {
            req = req.header("MCP-Protocol-Version", ver);
        }

        let response = req.send().await
            .map_err(|e| AppError::internal_error(format!("MCP notification {} failed: {}", method, e)))?;

        let status = response.status();
        // 202 Accepted (per spec) is the success case; some servers return 200.
        // Anything else is an error.
        if status.is_success() {
            return Ok(());
        }
        let body = response.text().await.unwrap_or_default();
        Err(AppError::internal_error(format!(
            "MCP notification {} returned HTTP {}: {}",
            method, status, body.chars().take(200).collect::<String>()
        )))
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

    /// Send a JSON-RPC request and return the deserialized result. Spec-conformant per
    /// MCP 2025-11-25:
    /// - Unique request id allocated from a monotonic counter (spec MUST NOT reuse).
    /// - Accept header lists BOTH `application/json` and `text/event-stream` (spec MUST).
    /// - `MCP-Protocol-Version` header sent after initialize completes (spec MUST).
    /// - SSE responses (Content-Type: text/event-stream) are parsed by iterating
    ///   all `data:` events and finding the one whose JSON-RPC id matches ours —
    ///   notifications/requests that may precede the response are logged and dropped.
    /// - HTTP 404 with our session id triggers a single reinitialize-and-retry
    ///   (spec MUST start a new session on 404).
    async fn request<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: Value,
    ) -> Result<T, AppError> {
        // Allow one reinitialize-and-retry on 404 (stale session). Initialize
        // itself must not recurse — guard via the method name.
        let allow_retry_on_404 = method != "initialize";
        match self.request_once::<T>(method, &params).await {
            Err(e) if allow_retry_on_404 && e.to_string().contains("HTTP 404") => {
                tracing::warn!(
                    "[mcp] server '{}' returned 404 for '{}' — stale session; reinitializing per MCP spec",
                    self.server_name, method
                );
                // Clear session so initialize doesn't echo it back
                if let Ok(mut g) = self.session_id.write() { *g = None; }
                self.do_initialize().await?;
                self.request_once::<T>(method, &params).await
            }
            other => other,
        }
    }

    /// Inner request — one attempt, no retry logic.
    async fn request_once<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: &Value,
    ) -> Result<T, AppError> {
        let id = self.next_id();
        let request_body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        let mut request = self.client
            .post(&self.base_url)
            // Per spec § Transports: client MUST advertise both content types so
            // the server can choose JSON or SSE.
            .header("Accept", "application/json, text/event-stream")
            .json(&request_body);

        if let Some(session_id) = self.get_session_id() {
            request = request.header("mcp-session-id", session_id);
        }
        // Per spec: MUST send MCP-Protocol-Version on all requests AFTER init.
        if let Some(ver) = self.get_protocol_version() {
            request = request.header("MCP-Protocol-Version", ver);
        }

        let response = request.send().await
            .map_err(|e| AppError::internal_error(format!("HTTP request failed: {}", e)))?;

        let status = response.status();

        if let Some(session_id) = response.headers().get("mcp-session-id") {
            if let Ok(s) = session_id.to_str() { self.set_session_id(s); }
        }

        let content_type = response.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let response_text = response.text().await
            .map_err(|e| AppError::internal_error(format!("Failed to read response: {}", e)))?;

        if !status.is_success() {
            return Err(AppError::internal_error(format!(
                "MCP server returned HTTP {}: {}",
                status, response_text.chars().take(200).collect::<String>()
            )));
        }

        // Two valid response shapes per spec § Transports:
        //  - Content-Type: application/json → single JSON-RPC response
        //  - Content-Type: text/event-stream → SSE stream with our response
        //    interleaved with optional notifications/requests
        let response_json = if content_type.starts_with("text/event-stream") {
            extract_response_by_id(&response_text, id)?
        } else {
            // Plain JSON. Tolerate trailing newline.
            serde_json::from_str(response_text.trim())
                .map_err(|e| AppError::internal_error(format!("Failed to parse JSON response: {}", e)))?
        };

        if let Some(error) = response_json.get("error") {
            return Err(AppError::internal_error(format!("MCP error: {}", error)));
        }

        let result = response_json.get("result")
            .ok_or_else(|| AppError::internal_error("MCP response missing 'result' field"))?;

        serde_json::from_value(result.clone())
            .map_err(|e| AppError::internal_error(format!("Failed to deserialize result: {}", e)))
    }

    /// Perform the initialize handshake. Used by both `connect()` and the
    /// 404-recovery path. Stores the negotiated protocol version and sends
    /// the required `notifications/initialized` notification afterward.
    async fn do_initialize(&self) -> Result<(), AppError> {
        let capabilities = if self.sampling_handler.is_some() {
            serde_json::json!({ "sampling": {}, "elicitation": {} })
        } else {
            serde_json::json!({ "elicitation": {} })
        };

        let init_params = serde_json::json!({
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": capabilities,
            "clientInfo": {
                "name": "ziee-chat",
                "version": env!("CARGO_PKG_VERSION"),
            },
        });

        let init_result: Value = self.request_once("initialize", &init_params).await?;

        // Per spec § version negotiation: server MUST respond with the same
        // version, or another version it supports. Record whichever we got
        // so subsequent requests send the right MCP-Protocol-Version header.
        let negotiated = init_result.get("protocolVersion")
            .and_then(|v| v.as_str())
            .unwrap_or(MCP_PROTOCOL_VERSION)
            .to_string();
        self.set_protocol_version(&negotiated);

        // MCP spec § Lifecycle: "After successful initialization, the client
        // MUST send an `initialized` notification to indicate it is ready to
        // begin normal operations."
        self.send_notification("notifications/initialized", serde_json::Value::Null).await?;

        Ok(())
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
        protocol_version: Option<String>,
        tool_call_id: i64,
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
            "id": tool_call_id,
            "method": "tools/call",
            "params": {
                "name": name,
                "arguments": arguments
            }
        });

        let mut req = stream_client
            .post(&url)
            // Per spec § Transports: client MUST advertise both content types.
            .header("Accept", "application/json, text/event-stream")
            .header("Content-Type", "application/json")
            .json(&request_body);

        let sid = get_sid();
        if let Some(ref s) = sid {
            req = req.header("mcp-session-id", s.as_str());
        }
        // Per spec: MUST send MCP-Protocol-Version on subsequent requests.
        if let Some(ref ver) = protocol_version {
            req = req.header("MCP-Protocol-Version", ver);
        }

        tracing::info!(
            "[sampling] tools/call → url={} headers={{Accept: application/json+SSE, Content-Type: application/json, mcp-session-id: {:?}, MCP-Protocol-Version: {:?}}} body={}",
            url, sid, protocol_version, request_body
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
        self.do_initialize().await?;
        self.connected = true;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), AppError> {
        // Per MCP spec § Session Management: "Clients that no longer need a
        // particular session SHOULD send an HTTP DELETE to the MCP endpoint
        // with the MCP-Session-Id header, to explicitly terminate the session."
        if let Some(sid) = self.get_session_id() {
            let mut req = self.client.delete(&self.base_url)
                .header("mcp-session-id", &sid);
            if let Some(ver) = self.get_protocol_version() {
                req = req.header("MCP-Protocol-Version", ver);
            }
            match req.send().await {
                Ok(r) => {
                    let status = r.status();
                    // Spec: server MAY respond with 405 if it doesn't allow
                    // client-initiated termination. Treat as success.
                    if !status.is_success() && status.as_u16() != 405 {
                        tracing::warn!(
                            "[mcp] DELETE session on '{}' returned HTTP {}",
                            self.server_name, status
                        );
                    }
                }
                Err(e) => {
                    // Don't fail disconnect on transport errors — local cleanup must proceed.
                    tracing::warn!("[mcp] DELETE session failed for '{}': {}", self.server_name, e);
                }
            }
            if let Ok(mut g) = self.session_id.write() { *g = None; }
        }
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

        // Allocate a unique request id once, up front — used by both branches.
        let tool_call_id = self.next_id();
        let protocol_version = self.get_protocol_version();

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
            let pv             = protocol_version.clone();

            let (result_tx, result_rx) =
                tokio::sync::oneshot::channel::<Result<ToolResult, AppError>>();

            tokio::spawn(async move {
                let result = HttpMcpClient::call_tool_with_sampling(
                    handler,
                    stream_client,
                    url,
                    session_id_arc,
                    pv,
                    tool_call_id,
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
        let pv_owned             = protocol_version;

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
                "id": tool_call_id,
                "method": "tools/call",
                "params": {
                    "name": name_owned,
                    "arguments": arguments_owned
                }
            });

            let mut req = stream_client
                .post(&url)
                // Per spec § Transports: client MUST advertise both content types.
                .header("Accept", "application/json, text/event-stream")
                .header("Content-Type", "application/json")
                .json(&request_body);

            if let Some(s) = get_sid() {
                req = req.header("mcp-session-id", s);
            }
            if let Some(ref ver) = pv_owned {
                req = req.header("MCP-Protocol-Version", ver);
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

    async fn list_prompts(&mut self) -> Result<Vec<Prompt>, AppError> {
        if !self.is_connected() {
            return Err(AppError::internal_error("Not connected"));
        }

        #[derive(serde::Deserialize)]
        struct ListPromptsResult {
            #[serde(default)]
            prompts: Vec<Prompt>,
        }

        // Servers that didn't advertise `prompts` capability may return
        // error -32601 (Method not found). Map that to an empty list so
        // callers don't have to special-case it.
        match self.request::<ListPromptsResult>("prompts/list", serde_json::json!({})).await {
            Ok(r) => Ok(r.prompts),
            Err(e) if e.to_string().contains("-32601") => Ok(Vec::new()),
            Err(e) => Err(e),
        }
    }

    async fn get_prompt(
        &mut self,
        name: &str,
        arguments: Option<Value>,
    ) -> Result<PromptResult, AppError> {
        if !self.is_connected() {
            return Err(AppError::internal_error("Not connected"));
        }

        let mut params = serde_json::json!({ "name": name });
        if let Some(args) = arguments {
            params["arguments"] = args;
        }

        self.request::<PromptResult>("prompts/get", params).await
    }

    async fn ping(&mut self) -> Result<(), AppError> {
        if !self.is_connected() {
            return Err(AppError::internal_error("Not connected"));
        }
        // MCP spec § utilities/ping: empty params; server responds with
        // an empty result. We don't care about the body.
        let _: Value = self.request("ping", serde_json::json!({})).await?;
        Ok(())
    }
}

// Tests for this module live in tests/mcp/mod.rs (see test_call_tool_with_sampling_sse_roundtrip
// and test_call_tool_with_real_llm_sampling).
