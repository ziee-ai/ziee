// MCP connection-test handlers.
//
// Build an *ephemeral* MCP client from a candidate configuration, run the
// `initialize` handshake (+ a best-effort tool listing), then disconnect —
// without persisting anything or touching the shared session pool. Surfaces a
// structured { success, message, tool_count } so the UI can show pass/fail and
// the underlying error (timeout / 401 / bad command) the moment a server is
// added or edited.

use aide::transform::TransformOperation;
use axum::{Json, debug_handler, http::StatusCode};
use chrono::Utc;
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError},
    core::Repos,
    modules::{
        mcp::client::{
            auth::OAuthClientConfig, http::HttpMcpClient, stdio::StdioMcpClient, traits::McpClient,
        },
        permissions::{RequirePermissions, with_permission},
    },
};

use super::super::{
    models::{McpServer, TransportType, UsageMode},
    permissions::*,
    types::{TestMcpConnectionRequest, TestMcpConnectionResponse},
};

// =====================================================
// Pure helpers (unit-tested)
// =====================================================

/// Reject configurations that can't possibly connect before we spawn a process
/// or open a socket. SSE is intentionally allowed through here — it produces a
/// structured failure (with the deprecation message) in `run_connection_test`.
pub(crate) fn validate(req: &TestMcpConnectionRequest) -> Result<(), AppError> {
    match req.transport_type {
        TransportType::Stdio => {
            if req.command.as_ref().is_none_or(|c| c.trim().is_empty()) {
                return Err(AppError::bad_request(
                    "MISSING_COMMAND",
                    "Command is required for stdio transport",
                ));
            }
        }
        TransportType::Http => {
            if req.url.as_ref().is_none_or(|u| u.trim().is_empty()) {
                return Err(AppError::bad_request(
                    "MISSING_URL",
                    "URL is required for HTTP transport",
                ));
            }
        }
        TransportType::Sse => {}
    }
    // Reject interior-invalid header values up front (trailing whitespace is
    // trimmed, not rejected) so a bad Authorization token surfaces as a clear
    // 400 here instead of being silently dropped when we probe the server.
    if let Some(entries) = req.headers_entries.as_deref() {
        super::super::validate_header_entries(entries)?;
    }
    Ok(())
}

/// Resolve the request's structured entries into a flat
/// `serde_json::Value` map for the ephemeral probe. Per-entry semantic:
///
/// * `value: Some(v)` — use `v` verbatim (secret or not).
/// * `value: None` + `is_secret: true` + `existing` provided — fall
///   back to the existing server's decrypted value for that key
///   (mirrors the OAuth `client_secret` fallback in `resolve_oauth`).
/// * `value: None` otherwise — use empty string.
fn resolve_entries_for_probe(
    entries: Option<&[super::super::types::EnvVarEntry]>,
    existing_decrypted: Option<&serde_json::Value>,
) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    let Some(entries) = entries else {
        return serde_json::Value::Object(out);
    };
    for entry in entries {
        let value = match (entry.value.as_deref(), entry.is_secret) {
            (Some(v), _) => v.to_string(),
            (None, true) => existing_decrypted
                .and_then(|v| v.as_object())
                .and_then(|o| o.get(&entry.key))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            (None, false) => String::new(),
        };
        out.insert(entry.key.clone(), serde_json::Value::String(value));
    }
    serde_json::Value::Object(out)
}

/// Header variant of `resolve_entries_for_probe`. Identical logic;
/// separate signature because `EnvVarEntry` and `HeaderEntry` are
/// distinct types (see types.rs rationale).
fn resolve_header_entries_for_probe(
    entries: Option<&[super::super::types::HeaderEntry]>,
    existing_decrypted: Option<&serde_json::Value>,
) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    let Some(entries) = entries else {
        return serde_json::Value::Object(out);
    };
    for entry in entries {
        let value = match (entry.value.as_deref(), entry.is_secret) {
            (Some(v), _) => v.to_string(),
            (None, true) => existing_decrypted
                .and_then(|v| v.as_object())
                .and_then(|o| o.get(&entry.key))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            (None, false) => String::new(),
        };
        out.insert(entry.key.clone(), serde_json::Value::String(value));
    }
    serde_json::Value::Object(out)
}

/// Map the test request into a throwaway `McpServer` the client constructors
/// accept. `id` is `nil` — this row is never persisted nor pooled.
///
/// `existing` (when supplied) is the prior server fetched by the
/// handler — used to resolve `value: None` secret entries to their
/// stored decrypted value, mirroring the OAuth secret fallback.
pub(crate) fn build_ephemeral_server(
    req: &TestMcpConnectionRequest,
    user_id: Option<Uuid>,
    is_system: bool,
    existing: Option<&McpServer>,
) -> McpServer {
    let now = Utc::now();
    let env_map = resolve_entries_for_probe(
        req.environment_variables_entries.as_deref(),
        existing.map(|s| &s.environment_variables),
    );
    let header_map = resolve_header_entries_for_probe(
        req.headers_entries.as_deref(),
        existing.map(|s| &s.headers),
    );
    McpServer {
        id: Uuid::nil(),
        user_id,
        name: "connection-test".to_string(),
        display_name: "Connection Test".to_string(),
        description: None,
        enabled: true,
        is_system,
        is_built_in: false,
        transport_type: req.transport_type.clone(),
        command: req.command.clone(),
        args: req
            .args
            .as_ref()
            .map(|a| serde_json::json!(a))
            .unwrap_or_else(|| serde_json::json!([])),
        environment_variables: env_map,
        environment_variables_entries: Vec::new(),
        url: req.url.clone(),
        headers: header_map,
        headers_entries: Vec::new(),
        timeout_seconds: req.timeout_seconds.unwrap_or(30),
        supports_sampling: false,
        usage_mode: UsageMode::Auto,
        max_concurrent_sessions: None,
        // Connectivity probe only — never routed through the code_sandbox.
        run_in_sandbox: false,
        last_health_check_at: None,
        last_health_check_status: "untested".to_string(),
        last_health_check_reason: None,
        created_at: now,
        updated_at: now,
    }
}

fn failure(err: AppError) -> TestMcpConnectionResponse {
    TestMcpConnectionResponse {
        success: false,
        message: err.to_string(),
        tool_count: None,
    }
}

// =====================================================
// Core probe
// =====================================================

/// Connect an ephemeral client, count tools (best-effort), disconnect. Never
/// returns `Err` for a connection problem — a failed connection IS the result
/// the caller wants, reported as `success: false` with the underlying message.
pub(crate) async fn run_connection_test(
    server: McpServer,
    oauth: Option<OAuthClientConfig>,
) -> TestMcpConnectionResponse {
    let mut client: Box<dyn McpClient> = match server.transport_type {
        TransportType::Stdio => match StdioMcpClient::new(server) {
            Ok(c) => Box::new(c),
            Err(e) => return failure(e),
        },
        TransportType::Http => match HttpMcpClient::new_internal(server, None, oauth) {
            Ok(c) => Box::new(c),
            Err(e) => return failure(e),
        },
        TransportType::Sse => {
            return TestMcpConnectionResponse {
                success: false,
                message: "The SSE (HTTP+SSE) transport was deprecated in MCP 2025-03-26. \
                          Reconfigure this server to use the Streamable HTTP transport (\"http\")."
                    .to_string(),
                tool_count: None,
            };
        }
    };

    if let Err(e) = client.connect().await {
        return failure(e);
    }

    // The handshake already proved reachability; the tool count is a bonus.
    let tool_count = client.list_tools().await.ok().map(|t| t.len());

    // Tidy up; ignore disconnect errors — the test already succeeded.
    let _ = client.disconnect().await;

    TestMcpConnectionResponse {
        success: true,
        message: match tool_count {
            Some(n) => format!("Connected successfully — {n} tool(s) available"),
            None => "Connected successfully".to_string(),
        },
        tool_count,
    }
}

/// Resolve the OAuth config for the test.
///
/// 1. Credentials typed into the form (`req.oauth` with a non-empty secret) win.
/// 2. Otherwise, if `req.id` points at an existing server **whose stored URL
///    matches the URL under test**, reuse that server's stored secret. The URL
///    match is a deliberate guard: it stops a caller from pointing `url` at a
///    server they control while referencing a victim server's `id`, which would
///    otherwise exfiltrate that server's `client_secret`.
async fn resolve_oauth(
    req: &TestMcpConnectionRequest,
    existing: Option<&McpServer>,
) -> Result<Option<OAuthClientConfig>, AppError> {
    if let Some(oauth) = &req.oauth
        && !oauth.client_secret.is_empty()
    {
        return Ok(Some(OAuthClientConfig {
            client_id: oauth.client_id.clone(),
            client_secret: oauth.client_secret.clone(),
            scopes: oauth.scopes.clone(),
            resource: oauth.resource.clone(),
        }));
    }

    if let (Some(id), Some(existing)) = (req.id, existing)
        && existing.url.is_some()
        && existing.url == req.url
        && let Some(stored) = Repos.mcp.get_oauth_config(id).await?
    {
        return Ok(Some(stored.into_client_config()));
    }

    Ok(None)
}

// =====================================================
// Handlers
// =====================================================

/// Test a user/personal MCP server configuration. The stored OAuth secret is
/// only recovered for a server the caller owns (`get_user_server` is
/// ownership-scoped), and only when the URL is unchanged (see `resolve_oauth`).
#[debug_handler]
pub async fn test_user_connection(
    auth: RequirePermissions<(McpServersCreate,)>,
    Json(request): Json<TestMcpConnectionRequest>,
) -> ApiResult<Json<TestMcpConnectionResponse>> {
    validate(&request)?;

    let existing = match request.id {
        Some(id) => Repos.mcp.get_user_server(id, auth.user.id).await?,
        None => None,
    };
    let oauth = resolve_oauth(&request, existing.as_ref()).await?;

    let server =
        build_ephemeral_server(&request, Some(auth.user.id), false, existing.as_ref());
    let response = run_connection_test(server, oauth).await;
    // Record the outcome on the persisted server (if `request.id`
    // pointed at one). Lets the UI surface "last tested: …" outside
    // the enable flow too. Non-fatal — log on failure.
    if let Some(server_id) = request.id {
        let (status, reason) = if response.success {
            ("healthy", None)
        } else {
            ("unhealthy", Some(response.message.as_str()))
        };
        if let Err(e) = Repos.mcp.record_health_check(server_id, status, reason).await {
            tracing::warn!(error = ?e, server_id = %server_id, "mcp::health: failed to record test-connection result");
        }
    }
    Ok((StatusCode::OK, Json(response)))
}

pub fn test_user_connection_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersCreate,)>(op)
        .id("McpServer.testConnection")
        .tag("MCP Servers - Runtime")
        .summary("Test MCP server connection")
        .description(
            "Probe a candidate MCP server configuration without persisting it: run the \
             initialize handshake and report success/failure plus the discovered tool count.",
        )
        .response::<200, Json<TestMcpConnectionResponse>>()
        .response_with::<400, (), _>(|res| res.description("Invalid configuration"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Test a system MCP server configuration (admin). Stored OAuth secret reuse is
/// gated on a matching URL, same as the user variant.
#[debug_handler]
pub async fn test_system_connection(
    _auth: RequirePermissions<(McpServersAdminCreate,)>,
    Json(request): Json<TestMcpConnectionRequest>,
) -> ApiResult<Json<TestMcpConnectionResponse>> {
    validate(&request)?;

    let existing = match request.id {
        Some(id) => Repos.mcp.get_system_server(id).await?,
        None => None,
    };
    let oauth = resolve_oauth(&request, existing.as_ref()).await?;

    let server = build_ephemeral_server(&request, None, true, existing.as_ref());
    let response = run_connection_test(server, oauth).await;
    if let Some(server_id) = request.id {
        let (status, reason) = if response.success {
            ("healthy", None)
        } else {
            ("unhealthy", Some(response.message.as_str()))
        };
        if let Err(e) = Repos.mcp.record_health_check(server_id, status, reason).await {
            tracing::warn!(error = ?e, server_id = %server_id, "mcp::health: failed to record test-connection result");
        }
    }
    Ok((StatusCode::OK, Json(response)))
}

pub fn test_system_connection_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersAdminCreate,)>(op)
        .id("McpServerSystem.testConnection")
        .tag("MCP Servers - System")
        .summary("Test system MCP server connection")
        .description(
            "Probe a candidate system MCP server configuration without persisting it: run the \
             initialize handshake and report success/failure plus the discovered tool count.",
        )
        .response::<200, Json<TestMcpConnectionResponse>>()
        .response_with::<400, (), _>(|res| res.description("Invalid configuration"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

// =====================================================
// Unit tests
// =====================================================

#[cfg(test)]
mod tests {
    use super::*;

    use super::super::super::types::{EnvVarEntry, HeaderEntry};

    fn http_req() -> TestMcpConnectionRequest {
        TestMcpConnectionRequest {
            transport_type: TransportType::Http,
            command: None,
            args: None,
            environment_variables_entries: None,
            url: Some("https://example.com/mcp".to_string()),
            headers_entries: Some(vec![HeaderEntry {
                key: "x-test".to_string(),
                value: Some("1".to_string()),
                is_secret: false,
            }]),
            timeout_seconds: None,
            oauth: None,
            id: None,
        }
    }

    fn stdio_req() -> TestMcpConnectionRequest {
        TestMcpConnectionRequest {
            transport_type: TransportType::Stdio,
            command: Some("uvx".to_string()),
            args: Some(vec!["mcp-server-fetch".to_string()]),
            environment_variables_entries: Some(vec![EnvVarEntry {
                key: "FOO".to_string(),
                value: Some("bar".to_string()),
                is_secret: false,
            }]),
            url: None,
            headers_entries: None,
            timeout_seconds: Some(15),
            oauth: None,
            id: None,
        }
    }

    #[test]
    fn validate_rejects_stdio_without_command() {
        let mut req = stdio_req();
        req.command = None;
        assert!(validate(&req).is_err());
        req.command = Some("   ".to_string());
        assert!(validate(&req).is_err());
    }

    #[test]
    fn validate_rejects_http_without_url() {
        let mut req = http_req();
        req.url = None;
        assert!(validate(&req).is_err());
        req.url = Some(String::new());
        assert!(validate(&req).is_err());
    }

    #[test]
    fn validate_accepts_valid_configs() {
        assert!(validate(&http_req()).is_ok());
        assert!(validate(&stdio_req()).is_ok());
    }

    #[test]
    fn build_ephemeral_server_maps_stdio_fields() {
        let server = build_ephemeral_server(&stdio_req(), Some(Uuid::nil()), false, None);
        assert_eq!(server.transport_type, TransportType::Stdio);
        assert_eq!(server.command.as_deref(), Some("uvx"));
        assert_eq!(server.args, serde_json::json!(["mcp-server-fetch"]));
        assert_eq!(server.environment_variables, serde_json::json!({"FOO": "bar"}));
        assert_eq!(server.timeout_seconds, 15);
        assert!(!server.is_system);
        assert!(!server.is_built_in);
        assert!(server.enabled);
    }

    #[test]
    fn build_ephemeral_server_maps_http_fields_and_defaults() {
        let server = build_ephemeral_server(&http_req(), None, true, None);
        assert_eq!(server.transport_type, TransportType::Http);
        assert_eq!(server.url.as_deref(), Some("https://example.com/mcp"));
        assert_eq!(server.headers, serde_json::json!({"x-test": "1"}));
        // No args/env supplied → empty JSON containers, not null.
        assert_eq!(server.args, serde_json::json!([]));
        assert_eq!(server.environment_variables, serde_json::json!({}));
        // timeout_seconds omitted → default 30.
        assert_eq!(server.timeout_seconds, 30);
        assert!(server.is_system);
    }

    #[tokio::test]
    async fn run_connection_test_rejects_sse() {
        let mut req = http_req();
        req.transport_type = TransportType::Sse;
        let server = build_ephemeral_server(&req, None, false, None);
        let res = run_connection_test(server, None).await;
        assert!(!res.success);
        assert!(res.message.contains("deprecated"));
        assert!(res.tool_count.is_none());
    }

    #[tokio::test]
    async fn run_connection_test_reports_disallowed_stdio_command() {
        // A command outside the stdio allowlist fails at connect() before any
        // process spawn — a deterministic, network-free failure path.
        let mut req = stdio_req();
        req.command = Some("definitely-not-allowed-binary".to_string());
        let server = build_ephemeral_server(&req, None, false, None);
        let res = run_connection_test(server, None).await;
        assert!(!res.success);
        assert!(res.tool_count.is_none());
        assert!(!res.message.is_empty());
    }
}
