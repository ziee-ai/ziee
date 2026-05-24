use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::Value;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use super::auth::{self, OAuthClientConfig, StoredToken};
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

/// Protocol versions this client can interoperate with, newest first. The
/// negotiated version returned by a server's `initialize` MUST be one of
/// these or the client refuses the connection (spec § version negotiation).
const SUPPORTED_PROTOCOL_VERSIONS: &[&str] =
    &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05", "2024-10-07"];

/// Safety cap on `nextCursor` pagination loops (`list_tools` / `list_resources`
/// / `list_prompts`) so a buggy server that never drops the cursor can't spin
/// the client forever.
const MAX_PAGINATION_PAGES: usize = 1000;

/// Structural JSON-RPC id comparison. The spec allows ids to be a string or
/// an integer; our outgoing ids are always integers, but a server may echo
/// them back stringified — accept both so response correlation is robust.
fn json_id_eq(id: &Value, expected: i64) -> bool {
    match id {
        Value::Number(n) => n.as_i64() == Some(expected),
        Value::String(s) => s.parse::<i64>().ok() == Some(expected),
        _ => false,
    }
}

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
        // it's the one we're waiting for. Match structurally: JSON-RPC ids
        // may be a string OR a number (spec), and a legal-but-non-conformant
        // server may echo our numeric id as a string — `as_i64` alone would
        // miss that and we'd hang waiting for a response that already arrived.
        let id_matches = json
            .get("id")
            .map(|v| json_id_eq(v, expected_id))
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

/// Forward an MCP `notifications/progress` (received mid-call over the SSE
/// stream) to the chat UI as a `mcpToolProgress` named SSE event — mirrors
/// how `elicitation/create` is bridged to the browser. No-op when there is
/// no browser SSE sender (e.g. the runtime tool-test endpoint).
fn forward_progress_notification(
    sse_tx: &Option<tokio::sync::mpsc::UnboundedSender<Result<axum::response::sse::Event, std::convert::Infallible>>>,
    message_id: Option<uuid::Uuid>,
    server_name: &str,
    params: &Value,
) {
    let tx = match sse_tx {
        Some(t) => t,
        None => return,
    };
    let event_data = serde_json::json!({
        "message_id": message_id.map(|m| m.to_string()),
        "server": server_name,
        "progress_token": params.get("progressToken").cloned(),
        "progress": params.get("progress").and_then(|v| v.as_f64()).unwrap_or(0.0),
        "total": params.get("total").and_then(|v| v.as_f64()),
        "message": params.get("message").and_then(|v| v.as_str()),
    });
    let event = axum::response::sse::Event::default()
        .event("mcpToolProgress")
        .data(event_data.to_string());
    let _ = tx.send(Ok(event));
}

// ─── SSE stream resumability (MCP spec § Transports / Resumability) ──────────
//
// When a tool-call SSE stream drops before delivering the JSON-RPC response, a
// spec-conformant client reconnects via GET + `Last-Event-Id` and resumes,
// rather than failing the whole call. The server signals resumability by
// emitting SSE `id:` lines (a "priming event" — an `id:` with empty data — is
// enough). These defaults mirror the MCP TypeScript SDK
// (`client/streamableHttp.ts` DEFAULT_STREAMABLE_HTTP_RECONNECTION_OPTIONS).
const SSE_RECONNECT_INITIAL_MS: u64 = 1_000;
const SSE_RECONNECT_MAX_MS: u64 = 30_000;
const SSE_RECONNECT_GROW_FACTOR: f64 = 1.5;
const SSE_RECONNECT_MAX_RETRIES: u32 = 2;

/// Exponential backoff for the Nth (0-based) reconnect attempt, capped.
fn reconnect_delay_ms(attempt: u32) -> u64 {
    let d = (SSE_RECONNECT_INITIAL_MS as f64) * SSE_RECONNECT_GROW_FACTOR.powi(attempt as i32);
    (d as u64).min(SSE_RECONNECT_MAX_MS)
}

/// Extract the SSE `id:` field from an event block (last `id:` line wins per
/// the SSE spec). Returns `None` if the block carries no id — i.e. the server
/// isn't emitting resumable event ids, so we won't attempt a resume.
fn sse_event_id(event_block: &str) -> Option<String> {
    let mut id = None;
    for line in event_block.lines() {
        if let Some(rest) = line.strip_prefix("id:") {
            id = Some(rest.trim().to_string());
        }
    }
    id
}

/// Drain a single open SSE response on the standalone GET stream. Updates
/// `last_event_id` from each `id:` line and overrides `backoff_initial_ms`
/// when the server sends a `retry:` field. Resets `reconnect_attempt` to 0
/// on the first event of a successful open (per SDK semantics: a stream
/// that's working gets full reconnect budget back). Returns when the
/// stream closes or errors — the caller decides whether to reconnect.
async fn drain_standalone_sse(
    resp: reqwest::Response,
    server_name: &str,
    last_event_id: &mut Option<String>,
    backoff_initial_ms: &mut u64,
    reconnect_attempt: &mut u32,
) {
    let mut buf = String::new();
    let mut stream = resp.bytes_stream();
    let mut delivered_any_in_this_open = false;
    while let Some(chunk) = stream.next().await {
        let bytes = match chunk {
            Ok(b) => b,
            Err(e) => {
                tracing::info!(
                    "[mcp] '{server_name}' standalone GET-SSE read error: {e}"
                );
                return;
            }
        };
        let text = String::from_utf8_lossy(&bytes);
        // Normalize CRLF before the buffer ever holds it. Real servers
        // (Go/Java) emit `\r\n\r\n` event separators; a strict `\n\n`
        // parser would buffer forever.
        if text.contains('\r') {
            buf.push_str(&text.replace("\r\n", "\n").replace('\r', "\n"));
        } else {
            buf.push_str(&text);
        }
        while let Some(end) = buf.find("\n\n") {
            let event_block = buf[..end].to_string();
            buf.drain(..end + 2);
            if event_block.trim().is_empty() {
                continue;
            }
            if !delivered_any_in_this_open {
                // First event of a healthy connection — the SDK resets its
                // reconnect budget here. Our budget is consecutive
                // failures, so this restores it to full.
                *reconnect_attempt = 0;
                delivered_any_in_this_open = true;
            }
            if let Some(eid) = sse_event_id(&event_block) {
                *last_event_id = Some(eid);
            }
            if let Some(retry_ms) = sse_event_retry_ms(&event_block) {
                tracing::debug!(
                    "[mcp] '{server_name}' standalone GET-SSE server set retry={retry_ms}ms"
                );
                *backoff_initial_ms = retry_ms;
            }
            route_unsolicited_event(server_name, &event_block);
        }
    }
    tracing::debug!(
        "[mcp] '{server_name}' standalone GET-SSE closed (last_event_id={:?})",
        last_event_id
    );
}

/// Common reconnect tick: log the reason, check budget, sleep the backoff
/// delay, bump the counter. Returns `false` if the budget is exhausted (the
/// caller should `return` to exit the task) or `true` if the caller should
/// `continue` the reconnect loop.
async fn backoff_and_retry(
    server_name: &str,
    kind: &str,
    reason: &str,
    reconnect_attempt: &mut u32,
    backoff_initial_ms: u64,
) -> bool {
    if *reconnect_attempt >= SSE_RECONNECT_MAX_RETRIES {
        tracing::info!(
            "[mcp] '{server_name}' standalone GET-SSE giving up after {reconnect_attempt} \
             reconnect attempts ({kind}: {reason})"
        );
        return false;
    }
    let delay = backoff_delay_ms(backoff_initial_ms, *reconnect_attempt);
    *reconnect_attempt += 1;
    tracing::info!(
        "[mcp] '{server_name}' standalone GET-SSE {kind} ({reason}); \
         reconnecting in {delay}ms (attempt {reconnect_attempt})"
    );
    tokio::time::sleep(Duration::from_millis(delay)).await;
    true
}

/// Like `reconnect_delay_ms` but accepts a runtime-mutable initial — the
/// server's `retry:` field overrides the constant default.
fn backoff_delay_ms(initial_ms: u64, attempt: u32) -> u64 {
    let d = (initial_ms as f64) * SSE_RECONNECT_GROW_FACTOR.powi(attempt as i32);
    (d as u64).min(SSE_RECONNECT_MAX_MS)
}

/// Extract the SSE `retry:` field from an event block in milliseconds.
/// Returns `None` if the field is absent or malformed. Per the EventSource
/// spec, a server sends `retry:` to instruct clients to use that initial
/// delay before reconnecting.
fn sse_event_retry_ms(event_block: &str) -> Option<u64> {
    for line in event_block.lines() {
        if let Some(rest) = line.strip_prefix("retry:")
            && let Ok(n) = rest.trim().parse::<u64>() {
                return Some(n);
            }
    }
    None
}

/// Concatenate the `data:` lines of an SSE event block. Each `data:` line
/// contributes its content (sans the leading `data:` and optional single
/// space), and multiple `data:` lines in the same block are joined with `\n`
/// per the EventSource spec.
fn sse_event_data(event_block: &str) -> String {
    event_block
        .lines()
        .filter_map(|l| l.strip_prefix("data:").or_else(|| l.strip_prefix("data: ")))
        .map(str::trim_start)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Dispatch an unsolicited SSE event received on the standalone GET stream.
/// Today: parse the JSON-RPC envelope + log by method shape. Tomorrow: route
/// `sampling/createMessage` to the registered SamplingHandler,
/// `elicitation/create` to the elicitation channel, `notifications/progress`
/// to the progress consumer for the matching token, `notifications/cancelled`
/// into the in-flight cancel set — all those consumer channels are currently
/// per-call (set on `tools/call`), so there is no live consumer for an
/// unsolicited arrival outside a POST flow. The skeleton stays here so the
/// future wiring is mechanical, AND so production logs name the method
/// instead of dumping the raw JSON.
fn route_unsolicited_event(server_name: &str, event_block: &str) {
    let data = sse_event_data(event_block);
    if data.is_empty() {
        // Spec's "priming event" — `id:` with empty `data:` — used for
        // Last-Event-Id seeding (GET-resume support is a Phase-3 follow-up).
        return;
    }
    let parsed: Value = match serde_json::from_str(&data) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(
                "[mcp] standalone GET-SSE for '{server_name}' got non-JSON payload: {e}; data={}",
                data.chars().take(200).collect::<String>()
            );
            return;
        }
    };
    let method = parsed.get("method").and_then(|m| m.as_str());
    let has_id = parsed.get("id").is_some();
    match (method, has_id) {
        (Some("notifications/progress"), _) => {
            let pt = parsed
                .get("params")
                .and_then(|p| p.get("progressToken"))
                .map(|t| t.to_string())
                .unwrap_or_default();
            tracing::info!(
                "[mcp] '{server_name}' GET-SSE notifications/progress (token={pt}) — no consumer attached"
            );
        }
        (Some("notifications/cancelled"), _) => {
            tracing::info!(
                "[mcp] '{server_name}' GET-SSE notifications/cancelled — no consumer attached"
            );
        }
        (Some("notifications/message"), _) => {
            tracing::debug!("[mcp] '{server_name}' GET-SSE notifications/message");
        }
        (Some(m), _) if m.ends_with("/list_changed") => {
            tracing::debug!("[mcp] '{server_name}' GET-SSE {m}");
        }
        (Some("sampling/createMessage"), true) => {
            tracing::warn!(
                "[mcp] '{server_name}' GET-SSE sampling/createMessage received but no \
                 consumer is attached to the standalone stream — request will time out on \
                 the server side. Follow-up: wire SamplingHandler into the GET path."
            );
        }
        (Some("elicitation/create"), true) => {
            tracing::warn!(
                "[mcp] '{server_name}' GET-SSE elicitation/create received but no \
                 consumer is attached to the standalone stream — request will time out. \
                 Follow-up: wire elicitation channel into the GET path."
            );
        }
        (Some(m), _) => {
            tracing::debug!("[mcp] '{server_name}' GET-SSE {m} (no router branch)");
        }
        (None, _) => {
            tracing::debug!(
                "[mcp] '{server_name}' GET-SSE non-method JSON: {}",
                data.chars().take(200).collect::<String>()
            );
        }
    }
}

/// Attempt to resume a dropped tool-call SSE stream via `GET` +
/// `Last-Event-Id` (MCP resumability). Runs the bounded backoff retry loop
/// internally; returns the fresh streaming response on success, or `None` if
/// the stream isn't resumable (no event id was seen) or all retries are
/// exhausted. Because both SSE read loops *return* on the first result/error,
/// reaching a stream-EOF means the response has NOT arrived yet — so we never
/// need the SDK's `receivedResponse` guard here; `last_event_id.is_some()` is
/// exactly the SDK's `hasPrimingEvent`.
async fn try_resume_sse(
    stream_client: &Client,
    url: &str,
    session_id: Option<String>,
    protocol_version: &Option<String>,
    authorization: &Option<String>,
    last_event_id: &Option<String>,
    server_name: &str,
) -> Option<reqwest::Response> {
    let leid = last_event_id.as_ref()?;
    let mut attempt = 0u32;
    while attempt < SSE_RECONNECT_MAX_RETRIES {
        tokio::time::sleep(Duration::from_millis(reconnect_delay_ms(attempt))).await;
        attempt += 1;
        let mut req = stream_client
            .get(url)
            .header("Accept", "text/event-stream")
            .header("Last-Event-Id", leid.as_str());
        if let Some(s) = &session_id {
            req = req.header("mcp-session-id", s.as_str());
        }
        if let Some(ver) = protocol_version {
            req = req.header("MCP-Protocol-Version", ver);
        }
        if let Some(bearer) = authorization {
            req = req.header("Authorization", format!("Bearer {bearer}"));
        }
        match req.send().await {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!(
                    "[mcp] resumed SSE stream from '{}' via Last-Event-Id={} (attempt {})",
                    server_name, leid, attempt
                );
                return Some(resp);
            }
            Ok(resp) => tracing::warn!(
                "[mcp] SSE resume attempt {} for '{}' returned HTTP {}",
                attempt, server_name, resp.status()
            ),
            Err(e) => tracing::warn!(
                "[mcp] SSE resume attempt {} for '{}' failed: {}",
                attempt, server_name, e
            ),
        }
    }
    None
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
    /// OAuth 2.1 client_credentials config for an external server that requires
    /// it. `None` → no OAuth (most servers; our built-in server uses a static
    /// short-lived JWT instead). See `mcp/client/auth.rs`.
    oauth: Option<Arc<OAuthClientConfig>>,
    /// Cached bearer token (acquired lazily on the first 401) + the discovered
    /// token endpoint (remembered for refresh). Shared into the spawned
    /// tool-call tasks so they attach the same bearer.
    oauth_token: Arc<RwLock<Option<StoredToken>>>,
    oauth_token_endpoint: Arc<RwLock<Option<String>>>,
    /// Plan-3 Phase-3 (I2) — standalone GET-SSE consumer. After `initialized`
    /// the client opens a `GET` with `Accept: text/event-stream` to receive
    /// unsolicited server→client messages (notifications/progress beyond the
    /// active POST stream, server-initiated sampling, …). The task is aborted
    /// on `disconnect`; a 405 from the server is the documented "no standalone
    /// stream" signal and the task exits silently.
    get_sse_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl HttpMcpClient {
    pub fn new(server: McpServer) -> Result<Self, AppError> {
        Self::new_internal(server, None, None)
    }

    pub fn new_with_sampling(
        server: McpServer,
        handler: Arc<dyn SamplingHandler>,
    ) -> Result<Self, AppError> {
        Self::new_internal(server, Some(handler), None)
    }

    /// Construct a client that authenticates to an external server via the
    /// OAuth 2.1 `client_credentials` grant (acquired lazily on the first 401).
    pub fn new_with_oauth(
        server: McpServer,
        oauth: OAuthClientConfig,
    ) -> Result<Self, AppError> {
        Self::new_internal(server, None, Some(oauth))
    }

    pub(crate) fn new_internal(
        server: McpServer,
        sampling_handler: Option<Arc<dyn SamplingHandler>>,
        oauth: Option<OAuthClientConfig>,
    ) -> Result<Self, AppError> {
        if server.transport_type != TransportType::Http {
            return Err(AppError::bad_request("INVALID_TRANSPORT", "Only HTTP transport supported"));
        }

        let base_url = server.url.clone()
            .ok_or_else(|| AppError::bad_request("MISSING_URL", "Missing URL for HTTP transport"))?;

        let mut headers = reqwest::header::HeaderMap::new();
        if let Some(headers_map) = server.headers.as_object() {
            for (key, value) in headers_map {
                if let Some(val_str) = value.as_str()
                    && let (Ok(name), Ok(val)) = (
                        reqwest::header::HeaderName::from_bytes(key.as_bytes()),
                        reqwest::header::HeaderValue::from_str(val_str)
                    ) {
                        headers.insert(name, val);
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
            oauth: oauth.map(Arc::new),
            oauth_token: Arc::new(RwLock::new(None)),
            oauth_token_endpoint: Arc::new(RwLock::new(None)),
            get_sse_task: Mutex::new(None),
        })
    }

    /// Current cached bearer token, if configured and still valid.
    fn current_bearer(&self) -> Option<String> {
        let g = self.oauth_token.read().ok()?;
        g.as_ref()
            .filter(|t| t.is_valid())
            .map(|t| t.access_token.clone())
    }

    /// Run the OAuth client_credentials flow in response to a 401 challenge,
    /// caching the token + token endpoint. Returns the fresh access token.
    async fn acquire_oauth_token(&self, www_authenticate: &str) -> Result<String, AppError> {
        let config = self.oauth.as_ref().ok_or_else(|| {
            AppError::internal_error("server returned 401 but no OAuth client is configured")
        })?;
        // If we already have a (possibly expired) token + endpoint, refresh;
        // otherwise discover from the challenge.
        let cached = self.oauth_token.read().ok().and_then(|g| g.clone());
        let endpoint = self.oauth_token_endpoint.read().ok().and_then(|g| g.clone());
        let (token, endpoint) = match (cached, endpoint) {
            (Some(cur), Some(ep)) => {
                (auth::refresh_token(&self.client, &ep, config, &cur).await?, ep)
            }
            _ => auth::obtain_token_from_challenge(&self.client, www_authenticate, config).await?,
        };
        let access = token.access_token.clone();
        if let Ok(mut g) = self.oauth_token.write() {
            *g = Some(token);
        }
        if let Ok(mut g) = self.oauth_token_endpoint.write() {
            *g = Some(endpoint);
        }
        Ok(access)
    }

    // (Module-private helpers `route_unsolicited_event` + `sse_event_data`
    // live at module scope below — they don't capture `self`.)

    /// Plan-3 Phase-3 (I2) — open the standalone GET-SSE stream.
    ///
    /// After `initialized` the spec lets the server push unsolicited messages
    /// (notifications/progress on long-running work, server-initiated
    /// sampling, server→client requests) over a `GET` with `Accept:
    /// text/event-stream`. We spawn a background task that drains it; the
    /// task is owned by the client (`get_sse_task: Mutex<Option<JoinHandle>>`)
    /// and aborted by [`Self::abort_standalone_get_sse`] on disconnect.
    ///
    /// Conformance corners the task handles (mirrors the MCP TypeScript SDK
    /// `_startOrAuthSse` + `_handleSseStream` reconnect loop — see
    /// `.sec-audits/mcp-phase3-i2-get-sse-audit-2026-05-22.md`):
    ///
    ///   - **405 Method Not Allowed** → server doesn't offer the stream;
    ///     exit silently. Our built-in `/code-sandbox` does this (POST-only).
    ///   - **401** → if OAuth is configured, refresh the token (cached
    ///     endpoint → `auth::refresh_token` fast path, else discover via the
    ///     `WWW-Authenticate` challenge) and loop without counting it as a
    ///     reconnect attempt. Otherwise log + exit.
    ///   - **Other non-2xx / network error** → log + backoff + retry, up to
    ///     [`SSE_RECONNECT_MAX_RETRIES`] consecutive failures.
    ///   - **200 + text/event-stream** → drain events with CRLF
    ///     normalization (Go/Java servers emit `\r\n\r\n`), tracking
    ///     `Last-Event-Id` from `id:` lines and overriding the backoff
    ///     initial on a server `retry:` field. The reconnect counter resets
    ///     to 0 on the first delivered event of a connection so a working
    ///     stream that occasionally hiccups keeps its budget.
    ///   - **Stream end** → if still within retry budget, reconnect using
    ///     the cached `Last-Event-Id` (server can replay anything emitted
    ///     since); else exit.
    ///   - **Per-event router** → [`route_unsolicited_event`] parses the
    ///     JSON-RPC envelope + logs by method. Full consumer wiring
    ///     (sampling/elicitation/progress) is deferred behind the
    ///     no-current-consumer reality of this codebase, documented in the
    ///     audit doc + at the call sites in the POST-stream loops.
    fn spawn_standalone_get_sse(&self) {
        let url = self.base_url.clone();
        let server_name = self.server_name.clone();
        let stream_client = self.stream_client.clone();
        // Regular (timeout'd) client for the OAuth refresh round-trip — the
        // streaming client has no overall timeout and would never bail on a
        // wedged token endpoint.
        let client = self.client.clone();
        let session_id = self.session_id.clone();
        let protocol_version = self.negotiated_protocol_version.clone();
        let oauth = self.oauth.clone();
        let oauth_token = self.oauth_token.clone();
        let oauth_token_endpoint = self.oauth_token_endpoint.clone();

        let handle = tokio::spawn(async move {
            // Persistent across reconnects.
            let mut last_event_id: Option<String> = None;
            // SDK-default backoff curve; a server `retry:` field overrides the
            // initial. The grow-factor + cap are constants.
            let mut backoff_initial_ms: u64 = SSE_RECONNECT_INITIAL_MS;
            // Counts consecutive failures. Reset to 0 on first event of an
            // open stream — a long-lived stream that hiccups occasionally
            // gets fresh budget after each successful re-establish.
            let mut reconnect_attempt: u32 = 0;

            loop {
                // Re-snapshot the bearer on every iteration so an OAuth
                // refresh that happened on the POST flow (or that we just
                // ran on a 401 below) is picked up on the next connect.
                let bearer = oauth_token
                    .read()
                    .ok()
                    .and_then(|g| g.clone())
                    .filter(|t| t.is_valid())
                    .map(|t| t.access_token);

                let mut req = stream_client
                    .get(&url)
                    .header("Accept", "text/event-stream");
                if let Some(sid) = session_id.read().ok().and_then(|g| g.clone()) {
                    req = req.header("mcp-session-id", sid);
                }
                if let Some(pv) = protocol_version.read().ok().and_then(|g| g.clone()) {
                    req = req.header("MCP-Protocol-Version", pv);
                }
                if let Some(b) = &bearer {
                    req = req.header("Authorization", format!("Bearer {b}"));
                }
                if let Some(leid) = &last_event_id {
                    // Resume from the last delivered event. The server, if it
                    // kept a replay buffer, will deliver everything since.
                    req = req.header("Last-Event-Id", leid.as_str());
                }

                let resp = match req.send().await {
                    Ok(r) => r,
                    Err(e) => {
                        if !backoff_and_retry(
                            &server_name,
                            "send error",
                            &format!("{e}"),
                            &mut reconnect_attempt,
                            backoff_initial_ms,
                        )
                        .await
                        {
                            return;
                        }
                        continue;
                    }
                };
                let status = resp.status();

                // 405 → server doesn't offer the stream; exit silently.
                if status.as_u16() == 405 {
                    tracing::debug!(
                        "[mcp] standalone GET-SSE for '{server_name}': 405 (no standalone stream)"
                    );
                    return;
                }

                // 401 → if OAuth is configured, refresh the token and loop.
                // We do NOT count this as a reconnect attempt: token refresh
                // is a known recovery path, not a transport failure.
                if status.as_u16() == 401 {
                    if let Some(oauth_cfg) = &oauth {
                        let www = resp
                            .headers()
                            .get("www-authenticate")
                            .and_then(|v| v.to_str().ok())
                            .unwrap_or("")
                            .to_string();
                        tracing::info!(
                            "[mcp] '{server_name}' standalone GET-SSE returned 401; \
                             running OAuth refresh"
                        );
                        let cached =
                            oauth_token.read().ok().and_then(|g| g.clone());
                        let endpoint = oauth_token_endpoint
                            .read()
                            .ok()
                            .and_then(|g| g.clone());
                        let result = match (cached, endpoint) {
                            (Some(cur), Some(ep)) => {
                                auth::refresh_token(&client, &ep, oauth_cfg, &cur)
                                    .await
                                    .map(|t| (t, ep))
                            }
                            _ => {
                                auth::obtain_token_from_challenge(
                                    &client, &www, oauth_cfg,
                                )
                                .await
                            }
                        };
                        match result {
                            Ok((token, endpoint)) => {
                                if let Ok(mut g) = oauth_token.write() {
                                    *g = Some(token);
                                }
                                if let Ok(mut g) = oauth_token_endpoint.write() {
                                    *g = Some(endpoint);
                                }
                                continue;
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "[mcp] '{server_name}' standalone GET-SSE OAuth \
                                     refresh failed: {e}; exiting"
                                );
                                return;
                            }
                        }
                    }
                    tracing::warn!(
                        "[mcp] standalone GET-SSE for '{server_name}' returned 401 \
                         with no OAuth configured; exiting"
                    );
                    return;
                }

                // Other non-2xx → backoff + retry.
                if !status.is_success() {
                    if !backoff_and_retry(
                        &server_name,
                        "non-success",
                        &format!("HTTP {status}"),
                        &mut reconnect_attempt,
                        backoff_initial_ms,
                    )
                    .await
                    {
                        return;
                    }
                    continue;
                }

                // 200 + SSE — drain. The helper updates last_event_id from
                // `id:` lines + backoff_initial_ms from `retry:` fields, and
                // returns when the stream ends.
                drain_standalone_sse(
                    resp,
                    &server_name,
                    &mut last_event_id,
                    &mut backoff_initial_ms,
                    &mut reconnect_attempt,
                )
                .await;

                // Stream ended (server close or read error). If we have
                // budget AND we have a Last-Event-Id (i.e. the stream
                // delivered at least one event with an id at SOME point —
                // server is resumable), reconnect.
                if reconnect_attempt >= SSE_RECONNECT_MAX_RETRIES {
                    tracing::debug!(
                        "[mcp] '{server_name}' standalone GET-SSE exhausted \
                         reconnect budget ({reconnect_attempt} consecutive failures)"
                    );
                    return;
                }
                let delay = backoff_delay_ms(backoff_initial_ms, reconnect_attempt);
                reconnect_attempt += 1;
                tracing::debug!(
                    "[mcp] '{server_name}' standalone GET-SSE reconnecting in {delay}ms \
                     (attempt {reconnect_attempt}, Last-Event-Id={:?})",
                    last_event_id
                );
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
        });

        if let Ok(mut g) = self.get_sse_task.lock() {
            // If a previous task is still around (reconnect), abort it first.
            if let Some(old) = g.take() {
                old.abort();
            }
            *g = Some(handle);
        }
    }

    /// Abort the standalone GET-SSE task if running. Called from
    /// [`Self::disconnect`]; safe to call when no task is running.
    fn abort_standalone_get_sse(&self) {
        if let Ok(mut g) = self.get_sse_task.lock()
            && let Some(h) = g.take() {
                h.abort();
            }
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
        if let Some(bearer) = self.current_bearer() {
            req = req.header("Authorization", format!("Bearer {bearer}"));
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
        // At most one extra attempt: an OAuth-protected server's first request
        // 401s, we acquire a token, and retry with `Authorization: Bearer`.
        let mut oauth_retried = false;
        let (status, content_type, response_text) = loop {
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
            // Attach a cached OAuth bearer if we have a valid one.
            if let Some(bearer) = self.current_bearer() {
                request = request.header("Authorization", format!("Bearer {bearer}"));
            }

            let response = request.send().await
                .map_err(|e| AppError::internal_error(format!("HTTP request failed: {}", e)))?;

            let status = response.status();

            // OAuth 2.1: on 401 with a configured client, acquire a token from
            // the `WWW-Authenticate` challenge and retry the request once.
            if status.as_u16() == 401 && self.oauth.is_some() && !oauth_retried {
                oauth_retried = true;
                let www = response.headers()
                    .get("www-authenticate")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_string();
                tracing::info!(
                    "[mcp] server '{}' returned 401 for '{}'; running OAuth client_credentials flow",
                    self.server_name, method
                );
                self.acquire_oauth_token(&www).await?;
                continue;
            }

            if let Some(session_id) = response.headers().get("mcp-session-id")
                && let Ok(s) = session_id.to_str() { self.set_session_id(s); }

            let content_type = response.headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();

            let id_for_sse = id;
            let response_text = response.text().await
                .map_err(|e| AppError::internal_error(format!("Failed to read response: {}", e)))?;

            if !status.is_success() {
                return Err(AppError::internal_error(format!(
                    "MCP server returned HTTP {}: {}",
                    status, response_text.chars().take(200).collect::<String>()
                )));
            }
            break (status, content_type, (id_for_sse, response_text));
        };
        let (id, response_text) = response_text;
        let _ = status;

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

        // Per spec § version negotiation: the server responds with the same
        // version, or another version it supports. The client MUST validate
        // it against the versions it understands and disconnect on mismatch
        // (SDK client.ts:513-538) — we must NOT blindly trust + echo an
        // unknown version on every subsequent MCP-Protocol-Version header.
        let negotiated = init_result
            .get("protocolVersion")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                AppError::internal_error(
                    "MCP initialize response is missing `protocolVersion`",
                )
            })?
            .to_string();
        if !SUPPORTED_PROTOCOL_VERSIONS.contains(&negotiated.as_str()) {
            return Err(AppError::internal_error(format!(
                "MCP server negotiated unsupported protocol version {:?}; this client supports {:?}",
                negotiated, SUPPORTED_PROTOCOL_VERSIONS
            )));
        }
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
        bearer: Option<String>,
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
                "arguments": arguments,
                // Opt in to MCP progress notifications (spec § Progress): the
                // server MAY emit `notifications/progress` carrying this token
                // during a long-running call. We forward them to the chat UI.
                "_meta": { "progressToken": tool_call_id }
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
        if let Some(ref b) = bearer {
            req = req.header("Authorization", format!("Bearer {b}"));
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

        if let Some(sid) = response.headers().get("mcp-session-id")
            && let Ok(s) = sid.to_str() {
                set_sid(s);
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
        // Track the last SSE event id so a dropped stream can resume via
        // GET + Last-Event-Id (MCP resumability).
        let mut last_event_id: Option<String> = None;

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

                        // Remember the SSE event id (resumability priming).
                        if let Some(eid) = sse_event_id(&event_block) {
                            last_event_id = Some(eid);
                        }

                        // Extract data line from event block
                        let data_line = event_block.lines()
                            .find(|l| l.starts_with("data: "))
                            .map(|l| &l[6..]);

                        let data = match data_line {
                            Some(d) => d,
                            None => continue,
                        };
                        // Skip events with no data (priming / keep-alive).
                        if data.is_empty() { continue; }

                        let json: Value = match serde_json::from_str(data) {
                            Ok(v) => v,
                            Err(e) => {
                                tracing::warn!("Failed to parse MCP SSE event: {} — data: {}", e, &data[..data.len().min(200)]);
                                continue;
                            }
                        };

                        // Check if this is a server→client request (elicitation or sampling)
                        if let Some(method) = json.get("method").and_then(|m| m.as_str()) {
                            // --- Progress (MCP spec § Progress) ---
                            // A `notifications/progress` is a one-way notification
                            // (no response expected); forward to the chat UI and
                            // keep reading for the eventual tool result.
                            if method == "notifications/progress" {
                                let params = json.get("params").cloned().unwrap_or(Value::Null);
                                forward_progress_notification(&sse_tx, message_id, &server_name, &params);
                                continue;
                            }
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
                    // Network error mid-stream — try to resume via Last-Event-Id
                    // before giving up (MCP resumability).
                    if let Some(resp) = try_resume_sse(
                        &stream_client, &url, get_sid(), &protocol_version, &bearer, &last_event_id, &server_name,
                    ).await {
                        byte_stream = resp.bytes_stream();
                        buffer.clear();
                        continue;
                    }
                    return Err(AppError::internal_error(format!("SSE stream error: {}", e)));
                }
                None => {
                    // Stream ended before the tool result. If the server emitted
                    // event ids (resumable), reconnect via GET + Last-Event-Id.
                    if let Some(resp) = try_resume_sse(
                        &stream_client, &url, get_sid(), &protocol_version, &bearer, &last_event_id, &server_name,
                    ).await {
                        byte_stream = resp.bytes_stream();
                        buffer.clear();
                        continue;
                    }
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
        protocol_version: Option<String>,
        bearer: Option<String>,
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
        // Track the last SSE event id for resume-via-Last-Event-Id.
        let mut last_event_id: Option<String> = None;

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

                        // Remember the SSE event id (resumability priming).
                        if let Some(eid) = sse_event_id(&event_block) {
                            last_event_id = Some(eid);
                        }

                        let data_line = event_block.lines()
                            .find(|l| l.starts_with("data: "))
                            .map(|l| &l[6..]);

                        let data = match data_line {
                            Some(d) => d,
                            None => continue,
                        };
                        // Skip events with no data (priming / keep-alive).
                        if data.is_empty() { continue; }

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
                            // --- Progress (MCP spec § Progress) ---
                            if method == "notifications/progress" {
                                let params = json.get("params").cloned().unwrap_or(Value::Null);
                                forward_progress_notification(&sse_tx, message_id, &server_name, &params);
                                continue;
                            }
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
                    if let Some(resp) = try_resume_sse(
                        &stream_client, &url, get_sid(), &protocol_version, &bearer, &last_event_id, &server_name,
                    ).await {
                        byte_stream = resp.bytes_stream();
                        buffer.clear();
                        continue;
                    }
                    return Err(AppError::internal_error(format!("SSE stream error: {}", e)));
                }
                None => {
                    if let Some(resp) = try_resume_sse(
                        &stream_client, &url, get_sid(), &protocol_version, &bearer, &last_event_id, &server_name,
                    ).await {
                        byte_stream = resp.bytes_stream();
                        buffer.clear();
                        continue;
                    }
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
        // Plan-3 Phase-3 (I2) — open the standalone GET-SSE per MCP spec
        // § Transports. The stream carries unsolicited server→client messages
        // (progress notifications for in-flight work, server-initiated
        // sampling, etc.). Servers that don't offer it 405; our built-in
        // /code-sandbox does (POST-only route — axum returns 405 on GET) and
        // the task exits silently. The task is owned by the client and
        // aborted on `disconnect`.
        self.spawn_standalone_get_sse();
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), AppError> {
        // Plan-3 Phase-3 (I2): the standalone GET-SSE task is tied to this
        // session. Abort it FIRST so the in-flight HTTP read doesn't keep
        // the connection alive while we send the DELETE.
        self.abort_standalone_get_sse();

        // Per MCP spec § Session Management: "Clients that no longer need a
        // particular session SHOULD send an HTTP DELETE to the MCP endpoint
        // with the MCP-Session-Id header, to explicitly terminate the session."
        if let Some(sid) = self.get_session_id() {
            let mut req = self.client.delete(&self.base_url)
                .header("mcp-session-id", &sid);
            if let Some(ver) = self.get_protocol_version() {
                req = req.header("MCP-Protocol-Version", ver);
            }
            if let Some(bearer) = self.current_bearer() {
                req = req.header("Authorization", format!("Bearer {bearer}"));
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
            #[serde(default)]
            tools: Vec<Tool>,
            #[serde(default, rename = "nextCursor")]
            next_cursor: Option<String>,
        }

        // Follow `nextCursor` pagination (spec § Pagination) so servers with
        // more than one page of tools aren't silently truncated.
        let mut all = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..MAX_PAGINATION_PAGES {
            let params = match &cursor {
                Some(c) => serde_json::json!({ "cursor": c }),
                None => serde_json::json!({}),
            };
            let page: ListToolsResult = self.request("tools/list", params).await?;
            all.extend(page.tools);
            match page.next_cursor {
                Some(c) if !c.is_empty() => cursor = Some(c),
                _ => break,
            }
        }
        Ok(all)
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
        // Cached OAuth bearer (acquired at connect/initialize for OAuth servers).
        let bearer = self.current_bearer();

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
            let bearer1        = bearer.clone();

            let (result_tx, result_rx) =
                tokio::sync::oneshot::channel::<Result<ToolResult, AppError>>();

            tokio::spawn(async move {
                let result = HttpMcpClient::call_tool_with_sampling(
                    handler,
                    stream_client,
                    url,
                    session_id_arc,
                    pv,
                    bearer1,
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
        let bearer_owned         = bearer;

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
                    "arguments": arguments_owned,
                    // Opt in to MCP progress notifications (spec § Progress);
                    // forwarded to the chat UI as `mcpToolProgress` events.
                    "_meta": { "progressToken": tool_call_id }
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
            if let Some(ref b) = bearer_owned {
                req = req.header("Authorization", format!("Bearer {b}"));
            }

            let response = match req.send().await {
                Ok(r) => r,
                Err(e) => {
                    let _ = result_tx.send(Err(AppError::internal_error(format!("MCP request failed: {}", e))));
                    return;
                }
            };

            if let Some(sid) = response.headers().get("mcp-session-id")
                && let Ok(s) = sid.to_str() {
                    set_sid(s);
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
                    pv_owned,
                    bearer_owned,
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
            #[serde(default)]
            resources: Vec<Resource>,
            #[serde(default, rename = "nextCursor")]
            next_cursor: Option<String>,
        }

        let mut all = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..MAX_PAGINATION_PAGES {
            let params = match &cursor {
                Some(c) => serde_json::json!({ "cursor": c }),
                None => serde_json::json!({}),
            };
            let page: ListResourcesResult = self.request("resources/list", params).await?;
            all.extend(page.resources);
            match page.next_cursor {
                Some(c) if !c.is_empty() => cursor = Some(c),
                _ => break,
            }
        }
        Ok(all)
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
            #[serde(default, rename = "nextCursor")]
            next_cursor: Option<String>,
        }

        let mut all = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..MAX_PAGINATION_PAGES {
            let params = match &cursor {
                Some(c) => serde_json::json!({ "cursor": c }),
                None => serde_json::json!({}),
            };
            // Servers that didn't advertise `prompts` capability may return
            // error -32601 (Method not found). Map that to an empty list so
            // callers don't have to special-case it.
            match self
                .request::<ListPromptsResult>("prompts/list", params)
                .await
            {
                Ok(page) => {
                    all.extend(page.prompts);
                    match page.next_cursor {
                        Some(c) if !c.is_empty() => cursor = Some(c),
                        _ => break,
                    }
                }
                Err(e) if e.to_string().contains("-32601") => return Ok(Vec::new()),
                Err(e) => return Err(e),
            }
        }
        Ok(all)
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

    async fn cancel(&mut self, request_id: i64, reason: &str) -> Result<(), AppError> {
        // Fire-and-forget `notifications/cancelled` (MCP spec § cancellation).
        self.send_notification(
            "notifications/cancelled",
            serde_json::json!({ "requestId": request_id, "reason": reason }),
        )
        .await
    }
}

// Tests for this module live in tests/mcp/mod.rs (see test_call_tool_with_sampling_sse_roundtrip
// and test_call_tool_with_real_llm_sampling).
