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
    // body parse). Conversation-id parses next (always succeeds; may
    // be None for stateless methods like initialize/tools/list). Body
    // is last so a body-parse failure can never leak past auth.
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
    let id = req.id.clone();

    // Stateless methods: succeed WITHOUT a conversation context. The
    // MCP client probes these during server discovery before any
    // conversation is established (manager.rs:88-93 only injects the
    // header when a conversation_id exists).
    match req.method.as_str() {
        "initialize" => {
            return ok_response(
                id,
                json!({
                    "protocolVersion": "2025-11-25",
                    "capabilities": { "tools": {} },
                    "serverInfo": {
                        "name": "code_sandbox",
                        "version": env!("CARGO_PKG_VERSION"),
                    },
                }),
            );
        }
        "tools/list" => {
            return ok_response(id, json!({ "tools": tool_definitions() }));
        }
        _ => {}
    }

    // Past this point we're dispatching a stateful method (today: only
    // `tools/call`), which requires the conversation context.
    let conversation_id = match conversation_id {
        Some(c) => c,
        None => {
            return error_response(
                id,
                StatusCode::OK,
                JsonRpcError::invalid_params(format!(
                    "method `{}` requires the x-conversation-id header",
                    req.method
                )),
            );
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
                id,
                StatusCode::INTERNAL_SERVER_ERROR,
                JsonRpcError::internal(format!("build_context: {e:?}")),
            );
        }
    };

    match dispatch(&ctx, &req).await {
        Ok(value) => ok_response(id, value),
        Err(err) => error_response(id, StatusCode::OK, err),
    }
}

/// Dispatch the stateful methods (the stateless ones — `initialize`,
/// `tools/list` — are handled in `jsonrpc_handler` before this is
/// reached, so they don't need a SandboxContext).
async fn dispatch(
    ctx: &SandboxContext,
    req: &JsonRpcRequest,
) -> Result<Value, JsonRpcError> {
    match req.method.as_str() {
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

            // MCP spec: `content[]` is an array of typed content blocks
            // ({type:"text"}, {type:"resource_link"}, {type:"image"}, ...).
            // Tools whose result IS already an MCP content block (i.e. has
            // a `type` field matching a known block type) must be passed
            // through verbatim — wrapping such a result in a text block
            // would silently break spec-conformant clients walking
            // `content[].type == "resource_link"`. Other tools' results
            // are structured JSON; serialize as a text block so the LLM
            // can parse them.
            let content = mcp_content_blocks(&result);

            Ok(json!({
                "content": content,
                "isError": false,
                "structuredContent": result,
            }))
        }
        other => Err(JsonRpcError::method_not_found(other)),
    }
}

fn ok_response(id: Option<Value>, result: Value) -> (StatusCode, Json<JsonRpcResponse>) {
    (
        StatusCode::OK,
        Json(JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }),
    )
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

/// Build the MCP `content[]` array for a tool result.
///
/// If the tool already returned a typed MCP content block
/// (`{type:"resource_link", ...}`, `{type:"image", ...}`, etc.),
/// pass it through verbatim. Otherwise serialize the structured
/// result as JSON in a text block.
///
/// Why this matters: the MCP 2025-11-25 spec says `content[]` is an
/// array of typed blocks. Wrapping a resource_link inside a text
/// block would silently break clients that walk
/// `content[].type == "resource_link"` to find downloadable URIs.
fn mcp_content_blocks(result: &Value) -> Vec<Value> {
    const KNOWN_CONTENT_TYPES: &[&str] = &[
        "text",
        "image",
        "audio",
        "resource_link",
        "resource",
    ];
    if let Some(t) = result.get("type").and_then(|v| v.as_str()) {
        if KNOWN_CONTENT_TYPES.contains(&t) {
            return vec![result.clone()];
        }
    }
    vec![json!({
        "type": "text",
        "text": serde_json::to_string(result).unwrap_or_default(),
    })]
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

    // Download is per-conversation; require the header.
    let conversation_id = conversation_id.ok_or((
        StatusCode::BAD_REQUEST,
        "x-conversation-id header required for file download".to_string(),
    ))?;

    let workspace = workspace_for(&state, conversation_id);
    let path = crate::modules::code_sandbox::tools::files::canonicalize_in_workspace(
        &workspace, &q.filename,
    )
    .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid filename: {e:?}")))?;

    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, format!("read: {e}")))?;

    let content_type = mime_guess::from_path(&q.filename)
        .first_or_octet_stream()
        .essence_str()
        .to_string();

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", content_type)
        .header(
            "Content-Disposition",
            format!(
                "attachment; filename=\"{}\"",
                disposition_filename(&q.filename)
            ),
        )
        .body(Body::from(bytes))
        .unwrap())
}

fn basename(name: &str) -> String {
    name.rsplit('/').next().unwrap_or(name).to_string()
}

/// Sanitize a filename for use in the `Content-Disposition` header
/// value. RFC 6266 quoted-string disallows `"` and bare backslash;
/// non-ASCII would need RFC 5987 `filename*=` encoding which we don't
/// emit. Replace problematic characters with `_` so the header is
/// always a valid HTTP header value. The actual download bytes are
/// the file content; the filename here is just a hint.
fn disposition_filename(name: &str) -> String {
    let base = basename(name);
    base.chars()
        .map(|c| {
            if c == '"' || c == '\\' || c == '\r' || c == '\n' || !c.is_ascii() {
                '_'
            } else {
                c
            }
        })
        .collect()
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

    // Stage attachments to the host paths that `build_bwrap_argv` will
    // bind into the sandbox at `/home/sandboxuser/<filename>`. Without
    // this, `--ro-bind-try` silently no-ops on every attachment and the
    // shell inside `execute_command` cannot see them — even though the
    // tool description tells the LLM "Conversation attachments are
    // available in the working directory by their original filename."
    //
    // The stage dir lives OUTSIDE the per-conversation workspace bind
    // mount, so sandboxed code cannot mutate the originals via the
    // workspace path — the file is only reachable via the explicit
    // `--ro-bind` at the friendly name.
    //
    // Staging is idempotent: file bytes don't change once uploaded, so
    // we only write on first miss. Per-file failures are logged and
    // skipped — the LLM falls back to the `read_file` tool (which
    // reads via the storage trait directly) if a bind fails.
    stage_attachments(state, &files).await;

    Ok(SandboxContext {
        conversation_id,
        user_id,
        workspace,
        files: Arc::new(files),
    })
}

async fn stage_attachments(
    state: &CodeSandboxState,
    files: &[crate::modules::code_sandbox::models::ConversationFile],
) {
    if files.is_empty() {
        return;
    }
    let stage_dir = state.workspace_root.join("attachments");
    if let Err(e) = tokio::fs::create_dir_all(&stage_dir).await {
        tracing::warn!(
            error = %e,
            "code_sandbox: failed to create attachments stage dir; \
             shell-side attachment access will be unavailable"
        );
        return;
    }
    let storage = crate::modules::file::storage::manager::get_file_storage();
    for f in files {
        let stage_path = stage_dir.join(f.file_id.to_string());
        // Idempotent: file_id is content-addressed in practice and the
        // bytes don't change once uploaded.
        if tokio::fs::try_exists(&stage_path).await.unwrap_or(false) {
            continue;
        }
        let ext = attachment_extension(&f.filename);
        match storage.load_original(f.user_id, f.file_id, ext).await {
            Ok(bytes) => {
                if let Err(e) = tokio::fs::write(&stage_path, &bytes).await {
                    tracing::warn!(
                        file_id = %f.file_id,
                        filename = %f.filename,
                        error = %e,
                        "code_sandbox: stage attachment write failed"
                    );
                }
            }
            Err(e) => {
                tracing::warn!(
                    file_id = %f.file_id,
                    filename = %f.filename,
                    error = ?e,
                    "code_sandbox: stage attachment load failed"
                );
            }
        }
    }
}

/// Extract the extension a `FileStorage::load_original` call expects
/// for the given filename. Returns `"bin"` for files without an
/// extension (storage manager's fallback convention).
fn attachment_extension(filename: &str) -> &str {
    std::path::Path::new(filename)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("bin")
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
    // Descriptions carry runtime guidance the LLM uses to plan its
    // tool calls (rootfs is read-only → --user installs; line-number
    // format passes through read_file → edit_file; etc.). Kept verbose
    // on purpose; clients surface these directly to the model.
    json!([
        {
            "name": "execute_command",
            "description": "Execute a shell command in the sandbox.\n\
                \n\
                RUNTIMES: Python 3, R, Node.js 22 (TypeScript / ts-node globally installed).\n\
                \n\
                BEFORE INSTALLING A PACKAGE: Always check if it is already installed first.\n\
                - Python: python3 -c \"import <pkg>\" 2>/dev/null && echo installed || echo missing\n\
                - R: Rscript -e \"'<pkg>' %in% rownames(installed.packages())\"\n\
                Only install if the check shows the package is missing.\n\
                \n\
                INSTALLING EXTRA PACKAGES: The rootfs /usr is read-only, so you MUST always use \
                'pip install --user <pkg>' (NOT 'pip install <pkg>'). User-installed packages persist \
                across calls in this conversation.\n\
                \n\
                FILES: Conversation attachments are available in the working directory by their \
                original filenames.\n\
                TIMEOUT: 10 minutes. Output capped at 1 MiB per stream.",
            "inputSchema": {
                "type": "object",
                "required": ["command"],
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Shell command to execute (runs via /bin/bash -lc)"
                    }
                }
            }
        },
        {
            "name": "read_file",
            "description": "Read a file by filename. Searches first in the sandbox working directory, \
                then falls back to conversation attachments (loaded transparently from the user's \
                originals store). Use just the filename (e.g., 'data.csv'), not a full path. \
                Output includes 1-indexed line numbers in the format 'N: content' — these line numbers \
                feed directly into edit_file.",
            "inputSchema": {
                "type": "object",
                "required": ["filename"],
                "properties": {
                    "filename": {
                        "type": "string",
                        "description": "Filename to read (plain name only, e.g. 'data.csv')"
                    },
                    "start_line": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "First line to return (1-indexed, optional)"
                    },
                    "end_line": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "Last line to return (inclusive, optional)"
                    }
                }
            }
        },
        {
            "name": "write_file",
            "description": "Write content to a file in the sandbox working directory. Creates or \
                overwrites the file (and any intermediate parent directories). Use a plain filename \
                (e.g., 'script.py'). If the resulting file needs to be shared with the user or passed \
                to another MCP server, call get_resource_link.",
            "inputSchema": {
                "type": "object",
                "required": ["filename", "content"],
                "properties": {
                    "filename": {
                        "type": "string",
                        "description": "Filename to write (plain name, e.g. 'script.py')"
                    },
                    "content": {
                        "type": "string",
                        "description": "File content to write"
                    }
                }
            }
        },
        {
            "name": "edit_file",
            "description": "Edit a file by replacing lines start_line through end_line (1-indexed, \
                inclusive) with new_content. Call read_file first to identify the correct line range — \
                output is in 'N: content' format. \
                new_content may be an empty string to delete lines without replacement. \
                To append after the last line, set start_line = total_line_count + 1 \
                (e.g., for a 10-line file, use start_line=11, end_line=11). \
                Use a plain filename. If the resulting file needs to be shared with the user or passed \
                to another MCP server, call get_resource_link.",
            "inputSchema": {
                "type": "object",
                "required": ["filename", "start_line", "end_line", "new_content"],
                "properties": {
                    "filename": {
                        "type": "string",
                        "description": "Filename to edit (plain name, e.g. 'script.py')"
                    },
                    "start_line": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "First line to replace (1-indexed, inclusive). Use total_line_count+1 to append after last line."
                    },
                    "end_line": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "Last line to replace (1-indexed, inclusive). Ignored when appending."
                    },
                    "new_content": {
                        "type": "string",
                        "description": "Replacement content for the specified line range. May be empty to delete lines, or multi-line."
                    }
                }
            }
        },
        {
            "name": "list_files",
            "description": "List files in the sandbox working directory (root level only, sorted \
                alphabetically). Shows user-written files and any conversation attachments that have \
                been read into the workspace. Hidden files (.dotfiles) and sandbox-internal state \
                (.sandbox_*) are excluded.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "get_resource_link",
            "description": "Return a standard MCP resource_link for a file, either from the sandbox \
                working directory or from user-attached conversation files.\n\
                \n\
                Use this tool when an external MCP server needs a download URL to access a file:\n\
                - For files you produced in this session (via write_file, edit_file, or \
                execute_command): returns an absolute URL the MCP client will fetch and may save as a \
                permanent artifact (`is_saved: false`).\n\
                - For user-attached files (referenced by their original filename): returns a short-lived \
                JWT-signed download-with-token URL that can be passed directly to external MCP tools \
                (`is_saved: true`).\n\
                \n\
                Use a plain filename (e.g., 'report.pdf' or 'data.csv').",
            "inputSchema": {
                "type": "object",
                "required": ["filename"],
                "properties": {
                    "filename": {
                        "type": "string",
                        "description": "Filename of the file in the sandbox working directory"
                    },
                    "save_as": {
                        "type": "string",
                        "description": "Optional display name for the artifact (defaults to filename)"
                    }
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

    // ─── mcp_content_blocks ─────────────────────────────────────────

    #[test]
    fn content_blocks_passes_through_resource_link() {
        let result = json!({
            "type": "resource_link",
            "uri": "http://example/foo",
            "name": "foo",
        });
        let blocks = super::mcp_content_blocks(&result);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0]["type"], "resource_link");
        assert_eq!(blocks[0]["uri"], "http://example/foo");
        // Critically: NOT wrapped in a text block.
        assert!(blocks[0].get("text").is_none());
    }

    #[test]
    fn content_blocks_passes_through_image_audio_resource() {
        for t in &["image", "audio", "resource"] {
            let result = json!({ "type": t, "data": "..." });
            let blocks = super::mcp_content_blocks(&result);
            assert_eq!(blocks[0]["type"], *t, "{t} block must pass through");
        }
    }

    #[test]
    fn content_blocks_wraps_structured_results_in_text() {
        let result = json!({
            "stdout": "hello",
            "exit_code": 0,
        });
        let blocks = super::mcp_content_blocks(&result);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0]["type"], "text");
        let text = blocks[0]["text"].as_str().unwrap();
        // The serialized JSON should round-trip.
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed, result);
    }

    #[test]
    fn content_blocks_wraps_unknown_type_in_text() {
        // A tool result with an unrelated `type` field (e.g. user data
        // with a "type" key) should NOT be passed through as a content
        // block — it must be wrapped in a text block so the LLM sees
        // the structured data.
        let result = json!({ "type": "user-defined", "color": "blue" });
        let blocks = super::mcp_content_blocks(&result);
        assert_eq!(blocks[0]["type"], "text");
    }

    // ─── Content-Disposition filename sanitization ──────────────────

    #[test]
    fn disposition_filename_strips_quotes_and_backslashes() {
        let raw = r#"foo"bar\baz.txt"#;
        let out = super::disposition_filename(raw);
        assert_eq!(out, "foo_bar_baz.txt");
    }

    #[test]
    fn disposition_filename_strips_non_ascii() {
        let raw = "café.txt";
        let out = super::disposition_filename(raw);
        assert!(out.is_ascii());
        assert!(out.ends_with(".txt"));
    }

    #[test]
    fn disposition_filename_strips_crlf() {
        let raw = "foo\r\nbar.txt";
        let out = super::disposition_filename(raw);
        assert!(!out.contains('\r'));
        assert!(!out.contains('\n'));
    }

    #[test]
    fn disposition_filename_takes_basename() {
        let raw = "sub/dir/foo.txt";
        let out = super::disposition_filename(raw);
        assert_eq!(out, "foo.txt");
    }

    // ─── attachment_extension ───────────────────────────────────────

    #[test]
    fn attachment_extension_extracts_simple() {
        assert_eq!(super::attachment_extension("data.csv"), "csv");
        assert_eq!(super::attachment_extension("foo.tar.gz"), "gz");
        assert_eq!(super::attachment_extension("IMG.PNG"), "PNG");
    }

    #[test]
    fn attachment_extension_falls_back_to_bin_for_no_extension() {
        // Must NOT return the whole filename (the naive
        // `rsplit('.').next().unwrap_or("bin")` pattern gets this wrong
        // — it returns "Makefile" for "Makefile" instead of "bin").
        assert_eq!(super::attachment_extension("Makefile"), "bin");
        assert_eq!(super::attachment_extension("README"), "bin");
        assert_eq!(super::attachment_extension(""), "bin");
    }

    #[test]
    fn attachment_extension_handles_dotfiles() {
        // ".bashrc" has no real extension — Path::extension returns
        // None for files whose name starts with a single dot.
        assert_eq!(super::attachment_extension(".bashrc"), "bin");
        // ".config.json" has extension "json".
        assert_eq!(super::attachment_extension(".config.json"), "json");
    }
}
