//! JSON-RPC dispatch + JWT validation + per-conversation concurrency
//! gate for the code_sandbox loopback HTTP surface.

use std::sync::Arc;

use axum::extract::Query;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use dashmap::DashMap;
use jsonwebtoken::{decode, DecodingKey, Validation};
use once_cell::sync::Lazy;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::core::repository::Repos;
use crate::modules::auth::jwt::Claims;
use crate::modules::code_sandbox::config;
use crate::modules::code_sandbox::types::{
    CodeSandboxState, JsonRpcError, JsonRpcRequest, JsonRpcResponse, SandboxContext,
};
use crate::modules::code_sandbox::{code_sandbox_server_id, tools};

/// Per-conversation mutex map. Two parallel tool calls in the same
/// conversation serialize; different conversations stay parallel.
/// Decorative `max_concurrent_sessions=1` on the server row gates only
/// the SAMPLING path (sandbox is supports_sampling=false), so we need
/// this gate ourselves.
///
/// The map grows monotonically as new conversation ids are seen.
/// Cleanup-on-conversation-delete is wired in a future change; in
/// practice each Arc<Mutex<()>> is tiny (~16 B).
static CONVERSATION_LOCKS: Lazy<DashMap<Uuid, Arc<Mutex<()>>>> = Lazy::new(DashMap::new);

fn conv_lock(conversation_id: Uuid) -> Arc<Mutex<()>> {
    CONVERSATION_LOCKS
        .entry(conversation_id)
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

// --------------------------------------------------------------------
// JSON-RPC entry point: POST /api/code-sandbox
// --------------------------------------------------------------------

pub async fn jsonrpc_handler(
    headers: HeaderMap,
    Json(req): Json<JsonRpcRequest>,
) -> (StatusCode, Json<JsonRpcResponse>) {
    let state = match config::get_state() {
        Some(s) => s,
        None => {
            return error_response(
                req.id,
                StatusCode::SERVICE_UNAVAILABLE,
                JsonRpcError::internal(
                    "code_sandbox not initialized (code_sandbox.enabled = false or boot probes failed)",
                ),
            );
        }
    };

    // Extract conversation_id + user_id (via JWT) BEFORE dispatch so we
    // can take the per-conversation mutex.
    let conversation_id = match extract_conversation_id(&headers) {
        Ok(cid) => cid,
        Err(status) => {
            return error_response(
                req.id,
                status,
                JsonRpcError::invalid_params("missing or malformed x-conversation-id header"),
            );
        }
    };

    let user_id = match extract_user_id_from_jwt(&headers, &state) {
        Ok(uid) => uid,
        Err((status, msg)) => {
            return error_response(req.id, status, JsonRpcError::invalid_params(msg));
        }
    };

    // Per-conversation serialization gate.
    let lock = conv_lock(conversation_id);
    let _guard = lock.lock().await;

    // Build SandboxContext with the workspace + conversation files.
    let ctx = match build_context(&state, conversation_id, user_id).await {
        Ok(c) => c,
        Err(e) => {
            return error_response(
                req.id,
                StatusCode::INTERNAL_SERVER_ERROR,
                JsonRpcError::internal(format!("build_context: {e:?}")),
            );
        }
    };

    let id = req.id.clone();
    let result = dispatch(&ctx, &req).await;
    match result {
        Ok(value) => (
            StatusCode::OK,
            Json(JsonRpcResponse {
                jsonrpc: "2.0",
                id,
                result: Some(value),
                error: None,
            }),
        ),
        Err(err) => error_response(id, StatusCode::OK, err),
    }
}

async fn dispatch(
    ctx: &SandboxContext,
    req: &JsonRpcRequest,
) -> Result<Value, JsonRpcError> {
    match req.method.as_str() {
        "initialize" => Ok(json!({
            "protocolVersion": "2025-11-25",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "code_sandbox", "version": env!("CARGO_PKG_VERSION") },
        })),
        "tools/list" => Ok(json!({ "tools": tool_definitions() })),
        "tools/call" => {
            #[derive(Deserialize)]
            struct CallParams {
                name: String,
                #[serde(default)]
                arguments: Value,
            }
            let p: CallParams = serde_json::from_value(req.params.clone())
                .map_err(|e| JsonRpcError::invalid_params(format!("tools/call params: {e}")))?;

            let result = invoke_tool(ctx, &p.name, &p.arguments)
                .await
                .map_err(|app_err| {
                    JsonRpcError::internal(format!(
                        "tool {} failed: {app_err:?}",
                        p.name
                    ))
                })?;

            Ok(json!({
                "content": [{ "type": "text", "text": serde_json::to_string(&result).unwrap_or_default() }],
                "isError": false,
                "structuredContent": result,
            }))
        }
        other => Err(JsonRpcError::method_not_found(other)),
    }
}

async fn invoke_tool(
    ctx: &SandboxContext,
    name: &str,
    args: &Value,
) -> Result<Value, crate::common::AppError> {
    match name {
        "execute_command" => {
            #[derive(Deserialize)]
            struct A {
                command: String,
            }
            let a: A = serde_json::from_value(args.clone())
                .map_err(invalid_tool_args)?;
            tools::execute::execute_command(ctx, &a.command).await
        }
        "read_file" => {
            #[derive(Deserialize)]
            struct A {
                filename: String,
                #[serde(default)]
                start_line: Option<usize>,
                #[serde(default)]
                end_line: Option<usize>,
            }
            let a: A = serde_json::from_value(args.clone())
                .map_err(invalid_tool_args)?;
            tools::files::read_file(ctx, &a.filename, a.start_line, a.end_line).await
        }
        "write_file" => {
            #[derive(Deserialize)]
            struct A {
                filename: String,
                content: String,
            }
            let a: A = serde_json::from_value(args.clone())
                .map_err(invalid_tool_args)?;
            tools::files::write_file(ctx, &a.filename, &a.content).await
        }
        "edit_file" => {
            #[derive(Deserialize)]
            struct A {
                filename: String,
                start_line: usize,
                end_line: usize,
                new_content: String,
            }
            let a: A = serde_json::from_value(args.clone())
                .map_err(invalid_tool_args)?;
            tools::files::edit_file(ctx, &a.filename, a.start_line, a.end_line, &a.new_content)
                .await
        }
        "list_files" => tools::files::list_files(ctx).await,
        "get_resource_link" => {
            #[derive(Deserialize)]
            struct A {
                filename: String,
                #[serde(default)]
                save_as: Option<String>,
            }
            let a: A = serde_json::from_value(args.clone())
                .map_err(invalid_tool_args)?;
            tools::files::get_resource_link(ctx, &a.filename, a.save_as.as_deref()).await
        }
        other => Err(crate::common::AppError::new(
            StatusCode::BAD_REQUEST,
            "UNKNOWN_TOOL",
            format!("unknown tool: {other}"),
        )),
    }
}

fn invalid_tool_args(e: serde_json::Error) -> crate::common::AppError {
    crate::common::AppError::new(
        StatusCode::BAD_REQUEST,
        "INVALID_TOOL_ARGS",
        format!("invalid tool arguments: {e}"),
    )
}

// --------------------------------------------------------------------
// GET /api/code-sandbox/file/download — workspace artifact serving.
// --------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct DownloadQuery {
    pub filename: String,
}

pub async fn download_handler(
    headers: HeaderMap,
    Query(q): Query<DownloadQuery>,
) -> Result<axum::response::Response, (StatusCode, String)> {
    use axum::body::Body;
    use axum::response::Response;

    let state = config::get_state().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "code_sandbox not initialized".to_string(),
    ))?;

    let conversation_id = extract_conversation_id(&headers)
        .map_err(|s| (s, "missing/invalid x-conversation-id".to_string()))?;
    let _user_id = extract_user_id_from_jwt(&headers, &state)
        .map_err(|(s, m)| (s, m))?;

    let workspace = workspace_for(&state, conversation_id);
    let path = crate::modules::code_sandbox::tools::files::canonicalize_in_workspace(
        &workspace, &q.filename,
    )
    .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid filename: {e:?}")))?;

    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, format!("read: {e}")))?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(
            "Content-Disposition",
            format!("attachment; filename=\"{}\"", basename(&q.filename)),
        )
        .body(Body::from(bytes))
        .unwrap())
}

fn basename(name: &str) -> String {
    name.rsplit('/').next().unwrap_or(name).to_string()
}

// --------------------------------------------------------------------
// Helpers
// --------------------------------------------------------------------

fn error_response(
    id: Option<Value>,
    status: StatusCode,
    err: JsonRpcError,
) -> (StatusCode, Json<JsonRpcResponse>) {
    (
        status,
        Json(JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(err),
        }),
    )
}

fn extract_conversation_id(headers: &HeaderMap) -> Result<Uuid, StatusCode> {
    let raw = headers
        .get("x-conversation-id")
        .ok_or(StatusCode::BAD_REQUEST)?
        .to_str()
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    Uuid::parse_str(raw.trim()).map_err(|_| StatusCode::BAD_REQUEST)
}

/// Read the Authorization: Bearer <jwt> header and verify against the
/// server's JWT secret. Returns the `sub` claim parsed as a Uuid.
fn extract_user_id_from_jwt(
    headers: &HeaderMap,
    _state: &CodeSandboxState,
) -> Result<Uuid, (StatusCode, String)> {
    let raw = headers
        .get("authorization")
        .or_else(|| headers.get("Authorization"))
        .ok_or((StatusCode::UNAUTHORIZED, "missing Authorization header".into()))?
        .to_str()
        .map_err(|_| (StatusCode::UNAUTHORIZED, "non-ASCII Authorization".into()))?;
    let token = raw
        .strip_prefix("Bearer ")
        .or_else(|| raw.strip_prefix("bearer "))
        .ok_or((
            StatusCode::UNAUTHORIZED,
            "Authorization must be Bearer <token>".into(),
        ))?;

    // The JWT secret lives in the server's loaded Config. We don't
    // have a direct hand-off, so we ask the file module for its cached
    // copy (initialized at server boot from the same config.jwt).
    let jwt_cfg = std::panic::catch_unwind(|| {
        crate::modules::file::config::get_jwt_config().clone()
    })
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "JWT config not initialized".to_string(),
        )
    })?;

    let mut validation = Validation::default();
    validation.set_audience(&[jwt_cfg.audience.clone()]);
    validation.set_issuer(&[jwt_cfg.issuer.clone()]);
    // The mcp manager's short-lived JWT for built-in servers uses a 5s
    // TTL; allow a small leeway for clock skew between request enter
    // and validation.
    validation.leeway = 2;

    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_cfg.secret.as_bytes()),
        &validation,
    )
    .map_err(|e| {
        (
            StatusCode::UNAUTHORIZED,
            format!("invalid Authorization token: {e}"),
        )
    })?;

    Uuid::parse_str(&data.claims.sub).map_err(|e| {
        (
            StatusCode::UNAUTHORIZED,
            format!("JWT sub is not a valid uuid: {e}"),
        )
    })
}

async fn build_context(
    state: &CodeSandboxState,
    conversation_id: Uuid,
    user_id: Uuid,
) -> Result<SandboxContext, crate::common::AppError> {
    let workspace = workspace_for(state, conversation_id);
    tokio::fs::create_dir_all(&workspace).await.map_err(|e| {
        crate::common::AppError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "WORKSPACE_INIT_FAILED",
            format!("mkdir workspace: {e}"),
        )
    })?;

    let files = Repos
        .code_sandbox
        .get_conversation_files(conversation_id)
        .await?;
    let _ = code_sandbox_server_id(); // keep symbol live for tests

    Ok(SandboxContext {
        conversation_id,
        user_id,
        workspace,
        files: Arc::new(files),
    })
}

fn workspace_for(state: &CodeSandboxState, conversation_id: Uuid) -> std::path::PathBuf {
    state
        .workspace_root
        .join(conversation_id.to_string())
}

// --------------------------------------------------------------------
// Tool catalog (single source of truth — snapshot-tested in Phase 9
// Tier 1 against a checked-in fixture).
// --------------------------------------------------------------------

fn tool_definitions() -> Value {
    json!([
        {
            "name": "execute_command",
            "description": "Run a shell command inside the sandbox. Returns stdout, stderr, exit_code. \
                Wall-clock timeout 600s. Output capped at 1 MiB per stream.",
            "inputSchema": {
                "type": "object",
                "required": ["command"],
                "properties": {
                    "command": { "type": "string", "description": "Shell command to execute" }
                }
            }
        },
        {
            "name": "read_file",
            "description": "Read a file from the sandbox workspace, line-numbered. Optional start/end line slice.",
            "inputSchema": {
                "type": "object",
                "required": ["filename"],
                "properties": {
                    "filename": { "type": "string", "description": "Workspace-relative path; no traversal allowed" },
                    "start_line": { "type": "integer", "minimum": 1 },
                    "end_line": { "type": "integer", "minimum": 1 }
                }
            }
        },
        {
            "name": "write_file",
            "description": "Create or overwrite a file in the sandbox workspace.",
            "inputSchema": {
                "type": "object",
                "required": ["filename", "content"],
                "properties": {
                    "filename": { "type": "string" },
                    "content": { "type": "string" }
                }
            }
        },
        {
            "name": "edit_file",
            "description": "Replace a range of lines in a file. start_line = len+1 appends.",
            "inputSchema": {
                "type": "object",
                "required": ["filename", "start_line", "end_line", "new_content"],
                "properties": {
                    "filename": { "type": "string" },
                    "start_line": { "type": "integer", "minimum": 1 },
                    "end_line": { "type": "integer", "minimum": 1 },
                    "new_content": { "type": "string" }
                }
            }
        },
        {
            "name": "list_files",
            "description": "List top-level files in the sandbox workspace (skips hidden + sandbox state).",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "get_resource_link",
            "description": "Returns an MCP resource_link for a file. User attachments are served via a \
                short-lived JWT-signed URL; workspace artifacts via the sandbox download route.",
            "inputSchema": {
                "type": "object",
                "required": ["filename"],
                "properties": {
                    "filename": { "type": "string" },
                    "save_as": { "type": "string", "description": "Optional display name override" }
                }
            }
        }
    ])
}
