//! JSON-RPC dispatch + per-conversation concurrency gate for the
//! code_sandbox loopback HTTP surface.
//!
//! Auth (JWT validation + `code_sandbox::execute` permission check)
//! is handled by the standard `RequirePermissions<(CodeSandboxExecute,)>`
//! axum extractor — same JWT secret the MCP manager
//! (`client/manager.rs:96-107`) signs with, plus the permission grant
//! migration 35 adds to the default Users group.

use std::sync::Arc;

use axum::extract::Query;
use axum::http::StatusCode;
use axum::Json;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::core::repository::Repos;
use crate::modules::code_sandbox::config;
use crate::modules::code_sandbox::permissions::CodeSandboxExecute;
use crate::modules::code_sandbox::types::{
    CodeSandboxState, ConversationIdHeader, JsonRpcError, JsonRpcRequest, JsonRpcResponse,
    SandboxContext,
};
use crate::modules::code_sandbox::{code_sandbox_server_id, tools};
use crate::modules::permissions::extractors::RequirePermissions;

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
    // Extractor order matters: auth runs first (rejects 401/403 before
    // body parse). Conversation-id parses next. Body is the last
    // extractor so a body-parse failure can never leak past auth.
    auth: RequirePermissions<(CodeSandboxExecute,)>,
    ConversationIdHeader(conversation_id): ConversationIdHeader,
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

    let user_id = auth.user.id;

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
    _auth: RequirePermissions<(CodeSandboxExecute,)>,
    ConversationIdHeader(conversation_id): ConversationIdHeader,
    Query(q): Query<DownloadQuery>,
) -> Result<axum::response::Response, (StatusCode, String)> {
    use axum::body::Body;
    use axum::response::Response;

    let state = config::get_state().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "code_sandbox not initialized".to_string(),
    ))?;

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

pub(crate) fn tool_definitions() -> Value {
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

// =====================================================================
// Tier 1 unit tests — tool catalog snapshot + JWT helper
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    /// Snapshot: the 6 tools' names + their required-arg sets. Bumping
    /// this is a deliberate signal that the public sandbox surface
    /// changed — schema bump territory.
    #[test]
    fn tool_definitions_snapshot() {
        let tools: Value = super::tool_definitions();
        let arr = tools.as_array().expect("tools is an array");
        assert_eq!(arr.len(), 6, "expected exactly 6 tools");

        let mut shape: Vec<(String, Vec<String>)> = arr
            .iter()
            .map(|t| {
                let name = t["name"].as_str().unwrap().to_string();
                let required: Vec<String> = t["inputSchema"]["required"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                (name, required)
            })
            .collect();
        shape.sort();

        let expected: Vec<(String, Vec<String>)> = vec![
            ("edit_file".into(), vec!["filename".into(), "start_line".into(), "end_line".into(), "new_content".into()]),
            ("execute_command".into(), vec!["command".into()]),
            ("get_resource_link".into(), vec!["filename".into()]),
            ("list_files".into(), vec![]),
            ("read_file".into(), vec!["filename".into()]),
            ("write_file".into(), vec!["filename".into(), "content".into()]),
        ];

        assert_eq!(
            shape, expected,
            "tool catalog changed — bump SANDBOX_ROOTFS_SCHEMA_VERSION + update this snapshot"
        );
    }

    #[test]
    fn tool_definitions_have_valid_input_schemas() {
        let tools = super::tool_definitions();
        for t in tools.as_array().unwrap() {
            let schema = &t["inputSchema"];
            assert_eq!(schema["type"], "object", "{} schema type", t["name"]);
            assert!(
                schema.get("properties").is_some(),
                "{} missing properties",
                t["name"]
            );
        }
    }

    // Conversation-id extraction tests moved to types.rs alongside
    // the ConversationIdHeader axum FromRequestParts implementation
    // they exercise.
}
