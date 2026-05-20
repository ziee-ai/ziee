//! Filesystem-side sandbox tools.
//!
//! All `filename` arguments are canonicalized into the per-conversation
//! workspace via [`canonicalize_in_workspace`]; any path that escapes
//! (via `..`, absolute path, or symlinks pointing outside) returns a
//! clean error instead of leaking host bytes.
//!
//! These tools operate on the HOST workspace directory (the same one
//! bound to `/home/sandboxuser` inside bwrap). That means subsequent
//! `execute_command` invocations see the writes immediately — there's
//! no copy-step.

use std::path::{Component, Path, PathBuf};

use axum::http::StatusCode;
use jsonwebtoken::{encode, EncodingKey, Header};
use serde_json::json;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::code_sandbox::config;
use crate::modules::code_sandbox::types::SandboxContext;
use crate::modules::file::types::DownloadTokenClaims;

/// Canonicalize a user-supplied filename to a path INSIDE the
/// conversation workspace. Rejects:
///   - Absolute paths (`/etc/passwd`)
///   - Paths containing `..` components (`../escape`, `a/../../b`)
///   - NUL bytes
///   - Path-traversal after symlink resolution
pub fn canonicalize_in_workspace(workspace: &Path, filename: &str) -> Result<PathBuf, AppError> {
    if filename.is_empty() {
        return Err(bad("filename is empty"));
    }
    if filename.contains('\0') {
        return Err(bad("filename contains NUL"));
    }
    let raw = Path::new(filename);
    if raw.is_absolute() {
        return Err(bad("absolute paths are not allowed"));
    }
    for c in raw.components() {
        match c {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(bad("path traversal (..) is not allowed"));
            }
        }
    }
    let candidate = workspace.join(raw);

    // Defense-in-depth: if the candidate exists, resolve symlinks and
    // re-confirm we're still under workspace. (For not-yet-existing
    // paths — like write_file targets — we resolve the parent.)
    let resolve_target = if candidate.exists() {
        candidate.clone()
    } else {
        candidate
            .parent()
            .unwrap_or(workspace)
            .to_path_buf()
    };
    let resolved = std::fs::canonicalize(&resolve_target).unwrap_or(resolve_target);
    let workspace_resolved = std::fs::canonicalize(workspace).unwrap_or_else(|_| workspace.to_path_buf());
    if !resolved.starts_with(&workspace_resolved) {
        return Err(bad("resolved path escapes workspace"));
    }

    Ok(workspace.join(raw))
}

fn bad(msg: impl Into<String>) -> AppError {
    AppError::new(StatusCode::BAD_REQUEST, "INVALID_FILENAME", msg)
}

fn io_err(detail: impl Into<String>) -> AppError {
    AppError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "WORKSPACE_IO_ERROR",
        detail,
    )
}

// ------------------------------------------------------------------
// read_file
// ------------------------------------------------------------------

pub async fn read_file(
    ctx: &SandboxContext,
    filename: &str,
    start_line: Option<usize>,
    end_line: Option<usize>,
) -> Result<serde_json::Value, AppError> {
    let path = canonicalize_in_workspace(&ctx.workspace, filename)?;
    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| io_err(format!("read {}: {e}", path.display())))?;

    let lines: Vec<&str> = content.lines().collect();
    let start = start_line.unwrap_or(1).max(1);
    let end = end_line.unwrap_or(lines.len()).min(lines.len());

    if start > lines.len() {
        return Ok(json!({ "text": "", "total_lines": lines.len() }));
    }

    let mut out = String::with_capacity(content.len().min(64 * 1024));
    for (i, line) in lines[start - 1..end].iter().enumerate() {
        out.push_str(&format!("{}: {}\n", start + i, line));
    }
    Ok(json!({ "text": out, "total_lines": lines.len() }))
}

// ------------------------------------------------------------------
// write_file
// ------------------------------------------------------------------

pub async fn write_file(
    ctx: &SandboxContext,
    filename: &str,
    content: &str,
) -> Result<serde_json::Value, AppError> {
    let path = canonicalize_in_workspace(&ctx.workspace, filename)?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| io_err(format!("mkdir parent {}: {e}", parent.display())))?;
    }
    tokio::fs::write(&path, content)
        .await
        .map_err(|e| io_err(format!("write {}: {e}", path.display())))?;
    Ok(json!({ "success": true, "bytes_written": content.len() }))
}

// ------------------------------------------------------------------
// edit_file: replace lines [start_line, end_line] with new_content.
// start_line = len + 1 → append.
// ------------------------------------------------------------------

pub async fn edit_file(
    ctx: &SandboxContext,
    filename: &str,
    start_line: usize,
    end_line: usize,
    new_content: &str,
) -> Result<serde_json::Value, AppError> {
    let path = canonicalize_in_workspace(&ctx.workspace, filename)?;
    let existing = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| io_err(format!("read {}: {e}", path.display())))?;
    let mut lines: Vec<String> = existing.lines().map(String::from).collect();
    let len = lines.len();

    if start_line == 0 {
        return Err(bad("start_line must be >= 1"));
    }
    if start_line > len + 1 {
        return Err(bad(format!(
            "start_line {start_line} is past end-of-file ({len} lines); use start_line={} to append",
            len + 1
        )));
    }
    if end_line < start_line {
        return Err(bad("end_line must be >= start_line"));
    }

    let append_mode = start_line == len + 1;
    let mut new_lines: Vec<String> =
        new_content.lines().map(String::from).collect();
    if append_mode {
        lines.extend(new_lines.drain(..));
    } else {
        let real_end = end_line.min(len);
        let _: Vec<String> = lines.drain((start_line - 1)..real_end).collect();
        for (i, l) in new_lines.into_iter().enumerate() {
            lines.insert(start_line - 1 + i, l);
        }
    }
    let mut out = lines.join("\n");
    if !existing.is_empty() && existing.ends_with('\n') {
        out.push('\n');
    } else if append_mode && !new_content.is_empty() {
        out.push('\n');
    }
    tokio::fs::write(&path, &out)
        .await
        .map_err(|e| io_err(format!("write {}: {e}", path.display())))?;
    Ok(json!({ "success": true }))
}

// ------------------------------------------------------------------
// list_files: top-level workspace listing (skip hidden + sandbox state).
// ------------------------------------------------------------------

pub async fn list_files(ctx: &SandboxContext) -> Result<serde_json::Value, AppError> {
    let mut entries = tokio::fs::read_dir(&ctx.workspace)
        .await
        .map_err(|e| io_err(format!("read_dir {}: {e}", ctx.workspace.display())))?;
    let mut out: Vec<serde_json::Value> = Vec::new();
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|e| io_err(format!("next_entry: {e}")))?
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(".sandbox_") || name.starts_with('.') {
            continue;
        }
        let meta = match entry.metadata().await {
            Ok(m) => m,
            Err(_) => continue,
        };
        out.push(json!({
            "name": name,
            "size": meta.len(),
            "is_file": meta.is_file(),
        }));
    }
    Ok(json!({ "files": out }))
}

// ------------------------------------------------------------------
// get_resource_link: JWT-signed URL for user attachments, plain URL
// for workspace artifacts. Returns an MCP `resource_link` content
// block shape.
// ------------------------------------------------------------------

pub async fn get_resource_link(
    ctx: &SandboxContext,
    filename: &str,
    save_as: Option<&str>,
) -> Result<serde_json::Value, AppError> {
    let _ = canonicalize_in_workspace(&ctx.workspace, filename)?;

    // First check whether the filename matches a known attachment by
    // name. If yes, return a JWT-signed download URL via the file
    // module's existing /api/files/{id}/download-with-token route.
    let attachment = ctx
        .files
        .iter()
        .find(|f| f.filename == filename && f.user_id == ctx.user_id);

    if let Some(att) = attachment {
        let token = sign_download_token(att.file_id, ctx.user_id)?;
        let url = format!("/api/files/{}/download-with-token?token={token}", att.file_id);
        let name = save_as.unwrap_or(&att.filename).to_string();
        return Ok(json!({
            "type": "resource_link",
            "uri": url,
            "name": name,
            "mimeType": att.mime_type,
            "description": "User-uploaded attachment (signed URL, expires shortly)"
        }));
    }

    // Otherwise treat as a workspace artifact, served by the sandbox
    // download route.
    let name = save_as.unwrap_or(filename).to_string();
    let url = format!(
        "/api/code-sandbox/file/download?filename={}",
        urlencoding_encode(filename)
    );
    Ok(json!({
        "type": "resource_link",
        "uri": url,
        "name": name,
        "description": "Sandbox workspace artifact"
    }))
}

/// Sign a short-lived JWT for the file-download endpoint. Reuses the
/// file module's `DownloadTokenClaims` so the receiver validates it
/// the same way.
fn sign_download_token(file_id: Uuid, user_id: Uuid) -> Result<String, AppError> {
    // We need the JWT secret. Pull it from the file module's cached
    // JwtConfig (initialized at server boot). The file module panics
    // if accessed before init; we wrap in catch_unwind to surface a
    // clean error instead.
    let jwt_cfg = std::panic::catch_unwind(|| {
        crate::modules::file::config::get_jwt_config().clone()
    })
    .map_err(|_| {
        AppError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "FILE_JWT_NOT_INITIALIZED",
            "file module JWT config missing; cannot sign download token",
        )
    })?;
    let now = chrono::Utc::now();
    let exp = (now + chrono::Duration::minutes(5)).timestamp() as usize;
    let claims = DownloadTokenClaims {
        file_id: file_id.to_string(),
        user_id: user_id.to_string(),
        exp,
        iat: now.timestamp() as usize,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_cfg.secret.as_bytes()),
    )
    .map_err(|e| {
        AppError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "JWT_SIGN_FAILED",
            format!("sign download token: {e}"),
        )
    })
}

/// Minimal RFC-3986 percent-encoder for the filename query param. We
/// avoid pulling the `urlencoding` crate just for this one site.
fn urlencoding_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
