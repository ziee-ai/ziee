//! JSON-RPC handler for the built-in files MCP server.
//!
//! Conversation-scoped: `user_id` from the injected JWT (`RequirePermissions`),
//! `conversation_id` from the `x-conversation-id` header (injected for built-in
//! servers by `mcp/client/manager.rs`). Reuses the JSON-RPC envelope types from
//! `code_sandbox` so we don't duplicate scaffolding. Reads serve the shared
//! `file::available_files` resolver + the file storage's extracted text; nothing
//! outside the conversation's effective file set is reachable.

use axum::{
    Json, debug_handler,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::common::AppError;
use crate::core::Repos;
use crate::modules::code_sandbox::types::{
    ConversationIdHeader, JsonRpcError, JsonRpcRequest, JsonRpcResponse,
};
use crate::modules::file::available_files::{AvailableFile, FileType, resolve_available_files};
use crate::modules::file::permissions::FilesRead;
use crate::modules::file::storage::manager::get_file_storage;
use crate::modules::permissions::RequirePermissions;

const DEFAULT_PAGE_LIMIT: usize = 10;
const DEFAULT_LINE_LIMIT: usize = 2000;
const GREP_MAX_MATCHES: usize = 200;
/// Per-call byte budget for `grep_files`. Bounds the work a single call
/// can do when scanning whole file bodies (a zero-match file is otherwise
/// read end-to-end with no match cap to stop it).
const GREP_MAX_SCAN_BYTES: usize = 16 * 1024 * 1024;

#[debug_handler]
pub async fn jsonrpc_handler(
    auth: RequirePermissions<(FilesRead,)>,
    ConversationIdHeader(conversation_id): ConversationIdHeader,
    body: axum::body::Bytes,
) -> Response {
    let raw: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            return error_response(
                None,
                StatusCode::BAD_REQUEST,
                JsonRpcError::parse_error(e.to_string()),
            );
        }
    };
    let req: JsonRpcRequest = match serde_json::from_value(raw) {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                None,
                StatusCode::BAD_REQUEST,
                JsonRpcError::invalid_request(e.to_string()),
            );
        }
    };

    if req.id.is_none() {
        return StatusCode::ACCEPTED.into_response();
    }

    let user_id = auth.user.id;
    let id = req.id.clone();

    match req.method.as_str() {
        "initialize" => ok_response(
            id,
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "files", "version": env!("CARGO_PKG_VERSION") },
            }),
        ),
        "tools/list" => ok_response(id, super::tools::tool_list()),
        "ping" => ok_response(id, json!({})),
        "tools/call" => {
            // tools/call requires a conversation context.
            let Some(conversation_id) = conversation_id else {
                return error_response(
                    id,
                    StatusCode::OK,
                    JsonRpcError::invalid_params(
                        "file tools require the x-conversation-id header".to_string(),
                    ),
                );
            };
            match dispatch_tool_call(user_id, conversation_id, &req.params).await {
                Ok(value) => ok_response(id, value),
                Err(e) => error_response(id, StatusCode::OK, app_error_to_jsonrpc(&e)),
            }
        }
        _ => error_response(id, StatusCode::OK, JsonRpcError::method_not_found(&req.method)),
    }
}

fn ok_response(id: Option<Value>, result: Value) -> Response {
    (
        StatusCode::OK,
        Json(JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }),
    )
        .into_response()
}

fn error_response(id: Option<Value>, http: StatusCode, err: JsonRpcError) -> Response {
    (
        http,
        Json(JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(err),
        }),
    )
        .into_response()
}

/// Map an `AppError` from a tool call onto the right JSON-RPC error class
/// instead of collapsing every failure to `internal` (-32603). Client-class
/// errors (bad args, unknown tool, not-found) become `invalid_params`
/// (-32602) / `method_not_found` (-32601); everything else stays `internal`.
fn app_error_to_jsonrpc(e: &AppError) -> JsonRpcError {
    let msg = e.to_string();
    match e.status_code() {
        400 if e.error_code() == "UNKNOWN_TOOL" => {
            JsonRpcError::method_not_found(&msg)
        }
        400 | 404 => JsonRpcError::invalid_params(msg),
        _ => JsonRpcError::internal(msg),
    }
}

/// Same NOT_FOUND-for-both semantics as code_sandbox: don't leak conversation
/// existence across tenants.
async fn assert_owns_conversation(conversation_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
    let owner = Repos
        .code_sandbox
        .get_conversation_user_id(conversation_id)
        .await?;
    if owner != Some(user_id) {
        return Err(AppError::not_found("Conversation"));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct ToolCallParams {
    name: String,
    #[serde(default)]
    arguments: Value,
}

async fn dispatch_tool_call(
    user_id: Uuid,
    conversation_id: Uuid,
    params: &Value,
) -> Result<Value, AppError> {
    assert_owns_conversation(conversation_id, user_id).await?;
    let call: ToolCallParams = serde_json::from_value(params.clone())
        .map_err(|e| AppError::bad_request("INVALID_PARAMS", format!("tools/call params: {e}")))?;

    let files = resolve_available_files(conversation_id, user_id).await?;

    match call.name.as_str() {
        "list_files" => list_files(&files),
        "read_file" => read_file(user_id, &files, &call.arguments).await,
        "grep_files" => grep_files(user_id, &files, &call.arguments).await,
        other => Err(AppError::bad_request(
            "UNKNOWN_TOOL",
            format!("files tool: {other}"),
        )),
    }
}

fn text_result(text: impl Into<String>, structured: Option<Value>) -> Value {
    let mut obj = json!({ "content": [{ "type": "text", "text": text.into() }] });
    if let Some(s) = structured {
        obj["structuredContent"] = s;
    }
    obj
}

fn list_files(files: &[AvailableFile]) -> Result<Value, AppError> {
    let manifest =
        serde_json::to_value(files).map_err(|e| AppError::internal_error(e.to_string()))?;
    let text = serde_json::to_string_pretty(&manifest).unwrap_or_default();
    Ok(text_result(text, Some(json!({ "files": manifest }))))
}

// ── read_file ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ReadArgs {
    #[serde(default)]
    id: Option<Uuid>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    offset: Option<usize>,
    #[serde(default)]
    limit: Option<usize>,
}

/// Resolve a target file from `id` (preferred) or `name` (only when unambiguous).
fn resolve_target<'a>(
    files: &'a [AvailableFile],
    id: Option<Uuid>,
    name: Option<&str>,
) -> Result<&'a AvailableFile, AppError> {
    if let Some(id) = id {
        return files
            .iter()
            .find(|f| f.id == id)
            .ok_or_else(|| AppError::not_found("File (no longer available in this conversation)"));
    }
    if let Some(name) = name {
        let matches: Vec<&AvailableFile> = files
            .iter()
            .filter(|f| f.name == name || f.aka.iter().any(|a| a == name))
            .collect();
        return match matches.as_slice() {
            [one] => Ok(one),
            [] => Err(AppError::not_found("File (no such name in this conversation)")),
            many => {
                let candidates: Vec<Value> = many
                    .iter()
                    .map(|f| json!({ "id": f.id, "name": f.name, "uploaded_at": f.uploaded_at }))
                    .collect();
                Err(AppError::bad_request(
                    "AMBIGUOUS_NAME",
                    format!(
                        "'{name}' matches {} files; call read_file with one of these ids: {}",
                        many.len(),
                        serde_json::to_string(&candidates).unwrap_or_default()
                    ),
                ))
            }
        };
    }
    Err(AppError::bad_request(
        "MISSING_TARGET",
        "read_file requires `id` or `name`",
    ))
}

async fn read_file(
    user_id: Uuid,
    files: &[AvailableFile],
    args: &Value,
) -> Result<Value, AppError> {
    let args: ReadArgs = serde_json::from_value(args.clone())
        .map_err(|e| AppError::bad_request("INVALID_ARGS", e.to_string()))?;
    let file = resolve_target(files, args.id, args.name.as_deref())?;
    let storage = get_file_storage();

    match file.file_type {
        FileType::Image => {
            // Return the image for vision (tool-result image block). Providers
            // whose tool results are text-only get this base64 image; the chat
            // layer can fall back to native re-injection there.
            let ext = extension_of(&file.name);
            let bytes = storage.load_original(user_id, file.id, ext).await?;
            use base64::Engine;
            let data = base64::engine::general_purpose::STANDARD.encode(&bytes);
            let mime = file
                .mime_type
                .clone()
                .unwrap_or_else(|| "image/png".to_string());
            Ok(json!({
                "content": [{ "type": "image", "data": data, "mimeType": mime }]
            }))
        }
        FileType::Binary => {
            let mime = file
                .mime_type
                .as_deref()
                .unwrap_or("application/octet-stream");
            let note = if mime == "application/pdf" {
                format!(
                    "[{} — no text layer (likely a scanned/image PDF); cannot be read as text]",
                    file.name
                )
            } else {
                format!("[{} ({}) — no extractable text]", file.name, mime)
            };
            Ok(text_result(note, None))
        }
        // Dispatch on the unit semantics implied by `file_type`, NOT on
        // `pages > 0`: a normally-processed text/code file always has
        // `pages == 1`, so a `pages > 0` test would route it to the
        // page reader and `read_text_lines` (the offset/limit-in-lines
        // path the tool advertises) would be dead.
        FileType::Text => {
            read_text_lines(&*storage, user_id, file, args.offset, args.limit).await
        }
        FileType::Document => {
            if file.pages > 0 {
                read_paginated(&*storage, user_id, file, args.offset, args.limit).await
            } else {
                read_text_lines(&*storage, user_id, file, args.offset, args.limit).await
            }
        }
    }
}

async fn read_paginated(
    storage: &dyn crate::modules::file::storage::FileStorage,
    user_id: Uuid,
    file: &AvailableFile,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<Value, AppError> {
    let total = file.pages.max(0) as usize;
    let start = offset.unwrap_or(0).min(total);
    let count = limit.unwrap_or(DEFAULT_PAGE_LIMIT).max(1);
    // saturating_add: `start + count` could overflow before `.min(total)`
    // since `limit` is fully model-controlled and unbounded.
    let end = start.saturating_add(count).min(total);

    let mut out = String::new();
    let mut missing: Vec<u32> = Vec::new();
    for page in start..end {
        // pages are stored 1-indexed.
        let page_num = (page + 1) as u32;
        match storage.load_text_page(user_id, file.id, page_num).await {
            Ok(text) => {
                out.push_str(&format!("\n--- {} · page {} ---\n", file.name, page + 1));
                out.push_str(&text);
                if !text.ends_with('\n') {
                    out.push('\n');
                }
            }
            Err(_) => {
                // Surface failed pages explicitly so a non-contiguous
                // range isn't presented as if it were contiguous.
                out.push_str(&format!(
                    "\n--- {} · page {} · [unreadable] ---\n",
                    file.name,
                    page + 1
                ));
                missing.push(page_num);
            }
        }
    }
    if out.is_empty() {
        out = format!("[{} — no text on pages {}..{}]", file.name, start + 1, end);
    }
    let truncated = end < total;
    if truncated {
        out.push_str(&format!(
            "\n[{} of {} pages shown; read_file(id, offset={}) for more]",
            end, total, end
        ));
    }
    Ok(text_result(
        out,
        Some(json!({
            "file_id": file.id, "name": file.name,
            "page_start": start + 1, "page_end": end, "total_pages": total,
            "missing_pages": missing,
        })),
    ))
}

async fn read_text_lines(
    storage: &dyn crate::modules::file::storage::FileStorage,
    user_id: Uuid,
    file: &AvailableFile,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<Value, AppError> {
    let ext = extension_of(&file.name);
    let bytes = storage.load_original(user_id, file.id, ext).await?;
    let content = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();
    let start = offset.unwrap_or(0).min(total);
    let count = limit.unwrap_or(DEFAULT_LINE_LIMIT).max(1);
    // saturating_add guards against `start + count` overflowing before
    // the `.min(total)` clamp (`limit` is model-controlled, unbounded).
    let end = start.saturating_add(count).min(total);
    let mut out = lines[start..end].join("\n");
    if end < total {
        out.push_str(&format!(
            "\n[lines {}..{} of {}; read_file(id, offset={}) for more]",
            start + 1,
            end,
            total,
            end
        ));
    }
    Ok(text_result(
        out,
        Some(json!({
            "file_id": file.id, "name": file.name,
            "line_start": start + 1, "line_end": end, "total_lines": total,
        })),
    ))
}

// ── grep_files ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct GrepArgs {
    pattern: String,
    #[serde(default)]
    id: Option<Uuid>,
    #[serde(default = "default_true")]
    ignore_case: bool,
}
fn default_true() -> bool {
    true
}

/// Lexical search: case-insensitive `regex` match over the extracted text, line
/// by line. An invalid regex
/// falls back to a literal (escaped) match so a malformed pattern from the model
/// doesn't error the whole call.
async fn grep_files(
    user_id: Uuid,
    files: &[AvailableFile],
    args: &Value,
) -> Result<Value, AppError> {
    let args: GrepArgs = serde_json::from_value(args.clone())
        .map_err(|e| AppError::bad_request("INVALID_ARGS", e.to_string()))?;
    if args.pattern.is_empty() {
        return Err(AppError::bad_request("INVALID_ARGS", "pattern must not be empty"));
    }
    let re = regex::RegexBuilder::new(&args.pattern)
        .case_insensitive(args.ignore_case)
        .build()
        .or_else(|_| {
            // Malformed regex → treat the pattern as a literal substring.
            regex::RegexBuilder::new(&regex::escape(&args.pattern))
                .case_insensitive(args.ignore_case)
                .build()
        })
        .map_err(|e| AppError::bad_request("INVALID_PATTERN", e.to_string()))?;
    let storage = get_file_storage();

    let targets: Vec<&AvailableFile> = files
        .iter()
        .filter(|f| args.id.map_or(true, |id| f.id == id))
        .filter(|f| f.text)
        .collect();

    let mut matches: Vec<Value> = Vec::new();
    // `truncated` is set whenever scanning stopped early (match cap or the
    // per-call byte budget) so the model can tell a capped result from an
    // exhaustive one. `missing` records pages that failed to load.
    let mut truncated = false;
    let mut scanned: usize = 0;
    let mut missing: Vec<Value> = Vec::new();
    'outer: for file in targets {
        let pages = if file.pages > 0 { file.pages as usize } else { 1 };
        for page in 0..pages {
            let text = if file.pages > 0 {
                match storage
                    .load_text_page(user_id, file.id, (page + 1) as u32)
                    .await
                {
                    Ok(t) => t,
                    Err(_) => {
                        // Record failed pages rather than silently swapping
                        // in an empty string (which would lose real matches).
                        missing.push(json!({
                            "file_id": file.id,
                            "name": file.name,
                            "page": page + 1,
                        }));
                        continue;
                    }
                }
            } else {
                let ext = extension_of(&file.name);
                match storage.load_original(user_id, file.id, ext).await {
                    Ok(b) => String::from_utf8_lossy(&b).into_owned(),
                    Err(_) => {
                        missing.push(json!({
                            "file_id": file.id,
                            "name": file.name,
                            "page": page + 1,
                        }));
                        continue;
                    }
                }
            };
            scanned = scanned.saturating_add(text.len());
            for (lineno, line) in text.lines().enumerate() {
                if re.is_match(line) {
                    matches.push(json!({
                        "file_id": file.id,
                        "name": file.name,
                        "page": page + 1,
                        "line": lineno + 1,
                        "text": line.trim(),
                    }));
                    if matches.len() >= GREP_MAX_MATCHES {
                        truncated = true;
                        break 'outer;
                    }
                }
            }
            if scanned >= GREP_MAX_SCAN_BYTES {
                truncated = true;
                break 'outer;
            }
        }
    }

    let mut summary = if matches.is_empty() {
        format!("No matches for '{}'.", args.pattern)
    } else {
        let lines: Vec<String> = matches
            .iter()
            .map(|m| {
                format!(
                    "{}:p{}:l{}: {}",
                    m["name"].as_str().unwrap_or(""),
                    m["page"],
                    m["line"],
                    m["text"].as_str().unwrap_or("")
                )
            })
            .collect();
        lines.join("\n")
    };
    if truncated {
        summary.push_str(&format!(
            "\n[showing first {} matches; results truncated — narrow the pattern or pass id]",
            GREP_MAX_MATCHES
        ));
    }
    if !missing.is_empty() {
        summary.push_str(&format!(
            "\n[{} page(s) could not be read and were skipped]",
            missing.len()
        ));
    }
    Ok(text_result(
        summary,
        Some(json!({
            "matches": matches,
            "truncated": truncated,
            "missing_pages": missing,
        })),
    ))
}

/// Derive the on-disk extension the SAME way the upload path does
/// (`upload.rs`: `filename.rsplit('.').next()...`). `Path::extension()`
/// returns `None` for a dot-less name (e.g. `photo`), which would look
/// for `{id}.bin` and 404 even though upload stored it as `{id}.photo`.
fn extension_of(filename: &str) -> &str {
    filename
        .rsplit('.')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("bin")
}
