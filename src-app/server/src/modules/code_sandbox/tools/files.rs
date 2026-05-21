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
    let content = load_file_content(ctx, filename).await?;
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

/// Load file contents with fallback to user-attached originals.
///
/// First tries the per-conversation workspace; if the file isn't there
/// and the filename matches one of the conversation's attachments,
/// loads it from the file module's `originals/<user_id>/<file_id>.<ext>`
/// storage. This is what makes `read_file("foo.csv")` work for a CSV
/// the user attached to the chat without first having to write it into
/// the workspace.
async fn load_file_content(ctx: &SandboxContext, filename: &str) -> Result<String, AppError> {
    let path = canonicalize_in_workspace(&ctx.workspace, filename)?;
    match tokio::fs::read_to_string(&path).await {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Try the user-attachment fallback before giving up.
            if let Some(att) = ctx
                .files
                .iter()
                .find(|f| f.filename == filename && f.user_id == ctx.user_id)
            {
                // Match the storage manager's "no extension → bin"
                // convention used in handlers::stage_attachments.
                // Naïve `rsplit('.').next()` returns the whole name for
                // files without a dot ("Makefile" → "Makefile"), so use
                // Path::extension which returns None in that case.
                let ext = std::path::Path::new(filename)
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("bin");
                let storage = crate::modules::file::storage::manager::get_file_storage();
                let bytes = storage
                    .load_original(ctx.user_id, att.file_id, ext)
                    .await
                    .map_err(|le| {
                        io_err(format!("load attachment {filename}: {le:?}"))
                    })?;
                return String::from_utf8(bytes).map_err(|_| {
                    AppError::new(
                        StatusCode::BAD_REQUEST,
                        "BINARY_FILE",
                        format!("{filename} is not valid UTF-8 (binary file)"),
                    )
                });
            }
            Err(io_err(format!("read {}: {e}", path.display())))
        }
        Err(e) => Err(io_err(format!("read {}: {e}", path.display()))),
    }
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
    // Same fallback as read_file: if the workspace file doesn't exist,
    // try loading from the user-attachment originals store. The first
    // edit copies the original into the workspace so subsequent edits
    // act on the workspace copy.
    let existing = load_file_content(ctx, filename).await?;
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
    // Ensure the workspace exists (it may not on the first call before
    // any write_file has run). Mirrors B's behavior so the LLM never
    // sees a confusing "read_dir failed: no such file" error on a
    // fresh conversation.
    if !ctx.workspace.exists() {
        tokio::fs::create_dir_all(&ctx.workspace)
            .await
            .map_err(|e| io_err(format!("mkdir workspace: {e}")))?;
    }
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
    // Stable alphabetical sort. Without this the LLM sees a
    // non-deterministic order each call (FS-dependent) which makes
    // the conversation harder to reason about.
    out.sort_by(|a, b| {
        let an = a["name"].as_str().unwrap_or("");
        let bn = b["name"].as_str().unwrap_or("");
        an.cmp(bn)
    });
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
    let path = canonicalize_in_workspace(&ctx.workspace, filename)?;

    // Base URL from the cached sandbox state's loopback URL — strip
    // the API path to get the origin. MCP clients on a different host
    // need an absolute URL to follow the link; relative paths only work
    // for same-origin consumers.
    let state = config::get_state().ok_or_else(|| {
        AppError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "SANDBOX_NOT_INITIALIZED",
            "code_sandbox state not initialized",
        )
    })?;
    let origin = state
        .loopback_url
        .trim_end_matches("/api/code-sandbox")
        .to_string();

    // First check whether the filename matches a known attachment by
    // name AND owned by the current user.
    let attachment = ctx
        .files
        .iter()
        .find(|f| f.filename == filename && f.user_id == ctx.user_id);

    if let Some(att) = attachment {
        let token = sign_download_token(att.file_id, ctx.user_id)?;
        let url = format!(
            "{origin}/api/files/{}/download-with-token?token={token}",
            att.file_id
        );
        let name = save_as.unwrap_or(&att.filename).to_string();
        return Ok(json!({
            "type": "resource_link",
            "uri": url,
            "name": name,
            "mimeType": att.mime_type,
            "description": "User-uploaded attachment (signed URL, expires shortly)",
            "is_saved": true,
        }));
    }

    // Workspace artifact path: confirm the file exists before returning
    // a link (otherwise the LLM hands the user a 404).
    if !path.exists() {
        return Err(AppError::new(
            StatusCode::NOT_FOUND,
            "FILE_NOT_FOUND",
            format!("{filename} not found in workspace"),
        ));
    }

    let mime = mime_guess::from_path(filename)
        .first_or_octet_stream()
        .essence_str()
        .to_string();
    let name = save_as.unwrap_or(filename).to_string();
    let url = format!(
        "{origin}/api/code-sandbox/file/download?filename={}",
        urlencoding_encode(filename)
    );
    Ok(json!({
        "type": "resource_link",
        "uri": url,
        "name": name,
        "mimeType": mime,
        "description": "Sandbox workspace artifact",
        "is_saved": false,
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

// =====================================================================
// Tier 1 unit tests — path traversal + file IO + edit_file arithmetic
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::code_sandbox::models::ConversationFile;
    use std::sync::Arc;
    use tempfile::TempDir;
    use uuid::Uuid;

    fn workspace() -> TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    fn ctx_for(tmp: &TempDir) -> SandboxContext {
        SandboxContext {
            conversation_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            workspace: tmp.path().to_path_buf(),
            files: Arc::new(Vec::<ConversationFile>::new()),
        }
    }

    // ─── path traversal ─────────────────────────────────────────────

    #[test]
    fn canonicalize_accepts_simple_names() {
        let tmp = workspace();
        assert!(canonicalize_in_workspace(tmp.path(), "foo.txt").is_ok());
        assert!(canonicalize_in_workspace(tmp.path(), "sub/foo.txt").is_ok());
        assert!(canonicalize_in_workspace(tmp.path(), "./sub/foo.txt").is_ok());
    }

    #[test]
    fn canonicalize_rejects_absolute_paths() {
        let tmp = workspace();
        let err = canonicalize_in_workspace(tmp.path(), "/etc/passwd")
            .expect_err("must reject absolute path");
        let msg = format!("{err:?}");
        assert!(msg.contains("absolute"), "msg: {msg}");
    }

    #[test]
    fn canonicalize_rejects_parent_dir_components() {
        let tmp = workspace();
        for path in &["../escape", "../../etc/passwd", "foo/../../../escape"] {
            let err = canonicalize_in_workspace(tmp.path(), path).expect_err(path);
            let msg = format!("{err:?}");
            assert!(msg.contains("traversal") || msg.contains(".."), "msg: {msg}");
        }
    }

    #[test]
    fn canonicalize_rejects_empty_and_nul() {
        let tmp = workspace();
        assert!(canonicalize_in_workspace(tmp.path(), "").is_err());
        assert!(canonicalize_in_workspace(tmp.path(), "foo\0bar").is_err());
    }

    // ─── read/write/list happy paths ───────────────────────────────

    #[tokio::test]
    async fn write_then_read_round_trip() {
        let tmp = workspace();
        let ctx = ctx_for(&tmp);
        write_file(&ctx, "hello.txt", "line1\nline2\nline3\n")
            .await
            .unwrap();
        let result = read_file(&ctx, "hello.txt", None, None).await.unwrap();
        let text = result["text"].as_str().unwrap();
        assert!(text.contains("1: line1"));
        assert!(text.contains("2: line2"));
        assert_eq!(result["total_lines"].as_u64().unwrap(), 3);
    }

    #[tokio::test]
    async fn read_file_slice() {
        let tmp = workspace();
        let ctx = ctx_for(&tmp);
        write_file(&ctx, "h.txt", "a\nb\nc\nd\ne\n").await.unwrap();
        let r = read_file(&ctx, "h.txt", Some(2), Some(4)).await.unwrap();
        let text = r["text"].as_str().unwrap();
        assert!(text.contains("2: b"));
        assert!(text.contains("4: d"));
        assert!(!text.contains("1: a"));
        assert!(!text.contains("5: e"));
    }

    #[tokio::test]
    async fn read_missing_file_returns_clean_error() {
        let tmp = workspace();
        let ctx = ctx_for(&tmp);
        let err = read_file(&ctx, "nope.txt", None, None)
            .await
            .expect_err("must error");
        let msg = format!("{err:?}");
        assert!(
            msg.contains("read") || msg.contains("WORKSPACE_IO_ERROR"),
            "msg: {msg}"
        );
    }

    #[tokio::test]
    async fn list_files_skips_hidden_and_sandbox_state() {
        let tmp = workspace();
        let ctx = ctx_for(&tmp);
        std::fs::write(tmp.path().join(".hidden"), "x").unwrap();
        std::fs::write(tmp.path().join(".sandbox_passwd"), "x").unwrap();
        std::fs::write(tmp.path().join("visible.txt"), "x").unwrap();
        let r = list_files(&ctx).await.unwrap();
        let names: Vec<String> = r["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["name"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(names, vec!["visible.txt".to_string()]);
    }

    // ─── edit_file arithmetic ─────────────────────────────────────

    #[tokio::test]
    async fn edit_file_replaces_inner_range() {
        let tmp = workspace();
        let ctx = ctx_for(&tmp);
        write_file(&ctx, "f.txt", "a\nb\nc\nd\n").await.unwrap();
        edit_file(&ctx, "f.txt", 2, 3, "X\nY").await.unwrap();
        let r = read_file(&ctx, "f.txt", None, None).await.unwrap();
        let text = r["text"].as_str().unwrap();
        assert!(text.contains("1: a"));
        assert!(text.contains("2: X"));
        assert!(text.contains("3: Y"));
        assert!(text.contains("4: d"));
        assert!(!text.contains("b") || text.contains("X"));
    }

    #[tokio::test]
    async fn edit_file_append_mode_when_start_is_len_plus_one() {
        let tmp = workspace();
        let ctx = ctx_for(&tmp);
        write_file(&ctx, "f.txt", "a\nb\n").await.unwrap();
        // len = 2 → start_line = 3 should append.
        edit_file(&ctx, "f.txt", 3, 3, "c\nd").await.unwrap();
        let r = read_file(&ctx, "f.txt", None, None).await.unwrap();
        assert_eq!(r["total_lines"].as_u64().unwrap(), 4);
    }

    #[tokio::test]
    async fn edit_file_past_end_is_rejected() {
        let tmp = workspace();
        let ctx = ctx_for(&tmp);
        write_file(&ctx, "f.txt", "a\nb\n").await.unwrap();
        let err = edit_file(&ctx, "f.txt", 99, 100, "x")
            .await
            .expect_err("past end");
        let msg = format!("{err:?}");
        assert!(msg.contains("past end-of-file"), "msg: {msg}");
    }

    #[tokio::test]
    async fn edit_file_rejects_inverted_range() {
        let tmp = workspace();
        let ctx = ctx_for(&tmp);
        write_file(&ctx, "f.txt", "a\nb\nc\n").await.unwrap();
        let err = edit_file(&ctx, "f.txt", 3, 2, "x")
            .await
            .expect_err("inverted");
        let msg = format!("{err:?}");
        assert!(msg.contains("end_line"), "msg: {msg}");
    }
}
