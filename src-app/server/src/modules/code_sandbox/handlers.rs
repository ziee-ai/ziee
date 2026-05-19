use aide::transform::TransformOperation;
use axum::extract::Query;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

use crate::common::AppError;
use crate::core::Repos;
use crate::modules::permissions::extractors::RequirePermissions;

use super::config::get_sandbox_config;
use super::permissions::CodeSandboxExecute;
use super::types::{ConversationIdHeader, JsonRpcRequest, JsonRpcResponse, SandboxContext, WorkspaceDownloadQuery};
use super::tools::{execute, files};

// =====================================================
// HTTP Entry Point
// =====================================================

pub async fn handle_mcp_request(
    auth: RequirePermissions<(CodeSandboxExecute,)>,
    ConversationIdHeader(conversation_id): ConversationIdHeader,
    Json(body): Json<JsonRpcRequest>,
) -> Json<JsonRpcResponse> {
    let cfg = get_sandbox_config();
    let ctx = SandboxContext {
        pool: Repos.pool().clone(),
        data_dir: cfg.data_dir.clone(),
        rootfs_path: cfg.rootfs_path.clone(),
        conversation_id,
        user_id: auth.user.id,
        base_url: cfg.base_url.clone(),
        jwt_secret: cfg.jwt_secret.clone(),
    };

    Json(dispatch(ctx, body).await)
}

pub fn handle_mcp_request_docs(op: TransformOperation) -> TransformOperation {
    op.id("CodeSandbox.rpc")
        .tag("Code Sandbox")
        .summary("MCP JSON-RPC endpoint for code sandbox tools")
        .response::<200, Json<JsonRpcResponse>>()
}

// =====================================================
// Workspace File Download
// =====================================================

pub async fn download_workspace_file(
    _auth: RequirePermissions<(CodeSandboxExecute,)>,
    ConversationIdHeader(conversation_id): ConversationIdHeader,
    Query(params): Query<WorkspaceDownloadQuery>,
) -> Result<Response, StatusCode> {
    let conversation_id = conversation_id.ok_or(StatusCode::BAD_REQUEST)?;
    let cfg = get_sandbox_config();
    let path = cfg
        .data_dir
        .join("sandboxes")
        .join(conversation_id.to_string())
        .join(&params.filename);

    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    let ext = std::path::Path::new(&params.filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin");
    let mime = mime_guess::from_ext(ext)
        .first_or_octet_stream()
        .to_string();
    let content_disposition = format!("attachment; filename=\"{}\"", params.filename);

    Ok((
        [
            (header::CONTENT_TYPE, mime),
            (header::CONTENT_DISPOSITION, content_disposition),
        ],
        bytes,
    )
        .into_response())
}

pub fn download_workspace_file_docs(op: TransformOperation) -> TransformOperation {
    op.id("CodeSandbox.downloadFile")
        .tag("Code Sandbox")
        .summary("Download sandbox workspace file")
        .description(
            "Download a file from the conversation sandbox workspace by filename. \
            Requires Authorization header (JWT) and X-Conversation-Id header.",
        )
        .response::<200, ()>()
        .response_with::<400, (), _>(|res| res.description("Missing conversation ID or filename"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("File not found"))
}

// =====================================================
// JSON-RPC Dispatch
// =====================================================

pub async fn dispatch(ctx: SandboxContext, req: JsonRpcRequest) -> JsonRpcResponse {
    let id = req.id.clone();
    match dispatch_inner(ctx, req).await {
        Ok(result) => JsonRpcResponse::ok(id, result),
        Err(e) => JsonRpcResponse::err(id, -32603, e.to_string()),
    }
}

async fn dispatch_inner(ctx: SandboxContext, req: JsonRpcRequest) -> Result<Value, AppError> {
    match req.method.as_str() {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": {
                "name": "ziee-code-sandbox",
                "version": "1.0.0"
            }
        })),

        "tools/list" => Ok(json!({
            "tools": tool_definitions()
        })),

        "tools/call" => {
            let name = req.params["name"]
                .as_str()
                .ok_or_else(|| AppError::bad_request("invalid_params", "missing tool name"))?;
            let arguments = req.params["arguments"].clone();
            call_tool(&ctx, name, arguments).await
        }

        _ => Err(AppError::bad_request(
            "method_not_found",
            format!("Unknown method: {}", req.method),
        )),
    }
}

async fn call_tool(ctx: &SandboxContext, name: &str, args: Value) -> Result<Value, AppError> {
    let conversation_id = ctx.conversation_id.ok_or_else(|| {
        AppError::bad_request(
            "missing_conversation",
            "Tool calls require a conversation context",
        )
    })?;

    match name {
        "execute_command" => {
            let command = args["command"]
                .as_str()
                .ok_or_else(|| AppError::bad_request("invalid_params", "missing 'command'"))?
                .to_string();

            let conv_files = Repos
                .code_sandbox
                .get_conversation_files(conversation_id, ctx.user_id)
                .await?;

            let out = execute::execute_command(
                &ctx.data_dir,
                &ctx.rootfs_path,
                conversation_id,
                ctx.user_id,
                conv_files,
                execute::ExecuteArgs { command },
            )
            .await?;

            let text = serde_json::to_string(&json!({
                "stdout": out.stdout,
                "stderr": out.stderr,
                "exit_code": out.exit_code
            }))
            .unwrap_or_default();

            Ok(tool_result(text, false))
        }

        "read_file" => {
            let filename = args["filename"]
                .as_str()
                .ok_or_else(|| AppError::bad_request("invalid_params", "missing 'filename'"))?;
            let start_line = args["start_line"].as_u64().map(|v| v as usize);
            let end_line = args["end_line"].as_u64().map(|v| v as usize);

            let conv_files = Repos
                .code_sandbox
                .get_conversation_files(conversation_id, ctx.user_id)
                .await?;

            let content = files::read_file(
                &ctx.data_dir,
                conversation_id,
                &conv_files,
                filename,
                start_line,
                end_line,
            )
            .await?;

            Ok(tool_result(content, false))
        }

        "write_file" => {
            let filename = args["filename"]
                .as_str()
                .ok_or_else(|| AppError::bad_request("invalid_params", "missing 'filename'"))?;
            let content = args["content"]
                .as_str()
                .ok_or_else(|| AppError::bad_request("invalid_params", "missing 'content'"))?;

            files::write_file(&ctx.data_dir, conversation_id, filename, content).await?;
            Ok(tool_result(r#"{"success":true}"#, false))
        }

        "edit_file" => {
            let filename = args["filename"]
                .as_str()
                .ok_or_else(|| AppError::bad_request("invalid_params", "missing 'filename'"))?;
            let start_line = args["start_line"]
                .as_u64()
                .ok_or_else(|| AppError::bad_request("invalid_params", "missing 'start_line'"))?
                as usize;
            let end_line = args["end_line"]
                .as_u64()
                .ok_or_else(|| AppError::bad_request("invalid_params", "missing 'end_line'"))?
                as usize;
            let new_content = args["new_content"]
                .as_str()
                .ok_or_else(|| AppError::bad_request("invalid_params", "missing 'new_content'"))?;

            let conv_files = Repos
                .code_sandbox
                .get_conversation_files(conversation_id, ctx.user_id)
                .await?;

            files::edit_file(&ctx.data_dir, conversation_id, &conv_files, filename, start_line, end_line, new_content)
                .await?;
            Ok(tool_result(r#"{"success":true}"#, false))
        }

        "list_files" => {
            let entries = files::list_files(&ctx.data_dir, conversation_id).await?;
            let text =
                serde_json::to_string(&json!({ "files": entries })).unwrap_or_default();
            Ok(tool_result(text, false))
        }

        "get_resource_link" => {
            let filename = args["filename"]
                .as_str()
                .ok_or_else(|| AppError::bad_request("invalid_params", "missing 'filename'"))?;
            let save_as = args["save_as"].as_str();

            let conv_files = Repos
                .code_sandbox
                .get_conversation_files(conversation_id, ctx.user_id)
                .await?;

            files::get_resource_link(ctx, &conv_files, filename, save_as).await
        }

        _ => Err(AppError::bad_request(
            "unknown_tool",
            format!("Unknown tool: {}", name),
        )),
    }
}

fn tool_result(text: impl Into<String>, is_error: bool) -> Value {
    json!({
        "content": [{"type": "text", "text": text.into()}],
        "isError": is_error
    })
}

fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "execute_command",
            "description": "Execute a shell command in the sandbox.\n\
                \n\
                RUNTIMES: Python 3, R, Node.js 22 (TypeScript/ts-node globally installed).\n\
                \n\
                BEFORE INSTALLING A PACKAGE: Always check if it is already installed first.\n\
                - Python: python3 -c \"import <pkg>\" 2>/dev/null && echo installed || echo missing\n\
                - R: Rscript -e \"'<pkg>' %in% rownames(installed.packages())\"\n\
                Only install if the check shows the package is missing.\n\
                \n\
                INSTALLING EXTRA PACKAGES: The rootfs /usr is read-only, so you MUST always use \
                'pip install --user <pkg>' (NOT 'pip install <pkg>'). \
                User-installed packages persist across calls in this conversation.\n\
                \n\
                FILES: Conversation attachments are available in the working directory by their original filenames.\n\
                TIMEOUT: 10 minutes.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Shell command to execute (runs via /bin/sh -c)"
                    }
                },
                "required": ["command"]
            }
        }),
        json!({
            "name": "read_file",
            "description": "Read a file by filename. \
                Searches first in the sandbox working directory, then falls back to conversation attachments. \
                Use just the filename (e.g., 'data.csv'), not a full path. \
                Output includes 1-indexed line numbers in the format 'N: content'. \
                Use these line numbers directly with edit_file.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "filename": {
                        "type": "string",
                        "description": "Filename to read (plain name only, e.g. 'data.csv')"
                    },
                    "start_line": {
                        "type": "integer",
                        "description": "First line to return (1-indexed, optional)"
                    },
                    "end_line": {
                        "type": "integer",
                        "description": "Last line to return (inclusive, optional)"
                    }
                },
                "required": ["filename"]
            }
        }),
        json!({
            "name": "write_file",
            "description": "Write content to a file in the sandbox working directory. \
                Creates or overwrites the file. Use a plain filename (e.g., 'script.py'). \
                If the resulting file needs to be shared with the user or passed to another MCP server, \
                call get_resource_link.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "filename": {
                        "type": "string",
                        "description": "Filename to write (plain name, e.g. 'script.py')"
                    },
                    "content": {
                        "type": "string",
                        "description": "File content to write"
                    }
                },
                "required": ["filename", "content"]
            }
        }),
        json!({
            "name": "edit_file",
            "description": "Edit a file by replacing lines start_line through end_line (1-indexed, inclusive) with new_content. \
                Call read_file first to identify the correct line range — output is in 'N: content' format. \
                new_content can be an empty string to delete lines without replacement. \
                To append after the last line, set start_line = total_line_count + 1 \
                (e.g., for a 10-line file, use start_line=11, end_line=11). \
                Use a plain filename (e.g., 'script.py'). \
                If the resulting file needs to be shared with the user or passed to another MCP server, \
                call get_resource_link.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "filename": {
                        "type": "string",
                        "description": "Filename to edit (plain name, e.g. 'script.py')"
                    },
                    "start_line": {
                        "type": "integer",
                        "description": "First line to replace (1-indexed, inclusive). Use total_line_count+1 to append after last line."
                    },
                    "end_line": {
                        "type": "integer",
                        "description": "Last line to replace (1-indexed, inclusive). Ignored when appending."
                    },
                    "new_content": {
                        "type": "string",
                        "description": "Replacement content for the specified line range. May be empty to delete lines, or multi-line."
                    }
                },
                "required": ["filename", "start_line", "end_line", "new_content"]
            }
        }),
        json!({
            "name": "list_files",
            "description": "List files in the sandbox working directory (root level only). \
                Shows all user-written files and conversation attachments that have been accessed.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }),
        json!({
            "name": "get_resource_link",
            "description": "Return a standard MCP resource_link for a file, either from the sandbox working directory \
                or from user-attached conversation files.\n\
                \n\
                Use this tool when an external MCP server needs a download URL to access a file:\n\
                - For files you produced in this session (via write_file, edit_file, or execute_command): \
                returns a resource_link that the MCP client will fetch, process, and save as a permanent artifact.\n\
                - For user-attached files (referenced by their original filename): returns a short-lived \
                download-with-token URL that can be passed directly to external MCP tools.\n\
                \n\
                Use a plain filename (e.g., 'report.pdf' or 'data.csv').",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "filename": {
                        "type": "string",
                        "description": "Filename of the file in the sandbox working directory"
                    },
                    "save_as": {
                        "type": "string",
                        "description": "Optional display name for the artifact (defaults to filename)"
                    }
                },
                "required": ["filename"]
            }
        }),
    ]
}
