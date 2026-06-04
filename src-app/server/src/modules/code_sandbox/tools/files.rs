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
///   - **Any** symlink along the path, INCLUDING dangling ones.
///
/// The dangling-symlink rejection is the critical defense. The
/// workspace is RW-bind-mounted into the sandbox at /home/sandboxuser,
/// so a sandboxed shell can `ln -s /etc/cron.d/x foo` (where the
/// target does not exist on the host). A naive `if candidate.exists()`
/// check would return false (the LINK exists, but the TARGET does
/// not), the canonical-path comparison would be skipped, and a
/// subsequent `tokio::fs::write` would FOLLOW the symlink at write
/// time and clobber the host file (cron job → host RCE). We defend
/// by walking each component and rejecting any that is itself a
/// symlink, regardless of whether its target resolves.
///
/// Returns the resolved (or assembled, if not-yet-existing) path so
/// callers do all subsequent IO via the verified path — avoiding
/// TOCTOU between this check and the file open.
pub fn canonicalize_in_workspace(workspace: &Path, filename: &str) -> Result<PathBuf, AppError> {
    if filename.is_empty() {
        return Err(bad("filename is empty"));
    }
    if filename.contains('\0') {
        return Err(bad("filename contains NUL"));
    }
    let raw = Path::new(filename);
    // Reject absolute paths AND leading `/` / `\` (which on Windows
    // `Path::is_absolute` doesn't consider absolute — it requires a drive
    // letter or UNC root — but in any sandboxed-path UX the user means
    // "absolute" by `/etc/passwd`). Sandbox semantics are POSIX-flavored
    // regardless of host OS, so a leading separator is always wrong.
    if raw.is_absolute()
        || filename.starts_with('/')
        || filename.starts_with('\\')
    {
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

    // Canonicalize the workspace root once. If it can't be
    // canonicalized (e.g. workspace dir was just created and the
    // filesystem hasn't propagated), fall back to the literal path —
    // we still get correctness because every component check below
    // compares against this base.
    let workspace_resolved = std::fs::canonicalize(workspace)
        .unwrap_or_else(|_| workspace.to_path_buf());

    // Walk each user-supplied component starting from the canonical
    // workspace. For each step:
    //   1. If the component is a symlink, REJECT (this is the
    //      dangling-symlink defense; symlinks at any depth could
    //      smuggle the final write/read to a host path).
    //   2. If the component doesn't exist yet, accept and keep
    //      building (write_file may be creating it).
    //   3. If it's a regular dir/file, descend.
    // After the walk, re-verify the assembled path still starts with
    // the canonical workspace (defense against any edge case we
    // missed).
    let mut cur = workspace_resolved.clone();
    for c in raw.components() {
        let Component::Normal(seg) = c else { continue };
        cur.push(seg);
        match std::fs::symlink_metadata(&cur) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(bad(format!(
                    "symlink component not allowed: {}",
                    seg.to_string_lossy()
                )));
            }
            Ok(_) => {} // regular dir/file: keep walking
            Err(_) => {} // doesn't exist yet — fine for write_file targets
        }
    }
    if !cur.starts_with(&workspace_resolved) {
        return Err(bad("assembled path escapes workspace"));
    }

    Ok(cur)
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

/// Cap on the size of content writable through the `write_file` tool.
/// Without this, an LLM (or a prompt-injection) could fill the host
/// disk: the in-sandbox `--fsize` rlimit only constrains writes from
/// INSIDE bwrap, and we apply `DefaultBodyLimit::disable()` globally,
/// so the axum json extractor accepts an arbitrarily large body. 32
/// MiB is generous for a code file but still bounded.
pub const WRITE_FILE_MAX_BYTES: usize = 32 * 1024 * 1024;

pub async fn write_file(
    ctx: &SandboxContext,
    filename: &str,
    content: &str,
) -> Result<serde_json::Value, AppError> {
    if content.len() > WRITE_FILE_MAX_BYTES {
        return Err(AppError::new(
            StatusCode::BAD_REQUEST,
            "CONTENT_TOO_LARGE",
            format!(
                "content {} bytes exceeds maximum {} bytes",
                content.len(),
                WRITE_FILE_MAX_BYTES
            ),
        ));
    }
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
        lines.append(&mut new_lines);
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
        // Skip entries whose names aren't valid UTF-8. Returning
        // them via to_string_lossy() would corrupt the name (`??`)
        // and a subsequent read_file/write_file would silently fail
        // to find the file. Better to omit than to mislead the LLM.
        let Some(name) = entry.file_name().to_str().map(|s| s.to_owned()) else {
            continue;
        };
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
    // Single source of truth shared with the MCP artifact-save pipeline
    // (mcp::chat_extension::mcp::file_download_origin): public_base_url when
    // set, else the pinned loopback origin already encoded in loopback_url.
    let loopback_origin = state.loopback_url.trim_end_matches("/api/code-sandbox");
    let origin = state.config.public_file_origin(loopback_origin);

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
            // File id so the UI inline preview fetches via the authenticated
            // /api/files/{id}/... path (same as the right-side panel).
            "file_id": att.file_id,
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
        iss: jwt_cfg.issuer.clone(),
        aud: crate::modules::file::types::DOWNLOAD_TOKEN_AUDIENCE.to_string(),
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

    // ─── write_file size cap (DoS defense) ──────────────────────────

    #[tokio::test]
    async fn write_file_rejects_oversized_content() {
        // SECURITY regression test: DefaultBodyLimit is disabled
        // server-wide and the in-sandbox --fsize rlimit doesn't cover
        // server-side writes. Without this cap, an LLM could fill the
        // host disk via a single write_file call.
        let tmp = workspace();
        let ctx = ctx_for(&tmp);
        let oversized = "x".repeat(WRITE_FILE_MAX_BYTES + 1);
        let err = write_file(&ctx, "huge.bin", &oversized)
            .await
            .expect_err("must reject oversized content");
        let msg = format!("{err:?}");
        assert!(msg.contains("CONTENT_TOO_LARGE"), "msg: {msg}");
    }

    // ─── dangling-symlink defense (sandbox escape regression) ────────

    #[test]
    #[cfg(unix)]
    fn canonicalize_rejects_dangling_symlink() {
        // SECURITY regression test for the sandbox-escape vector
        // documented in `canonicalize_in_workspace`. A sandboxed shell
        // can create a symlink pointing at a host path that does NOT
        // exist (e.g. /etc/cron.d/payload). Without this defense, a
        // follow-up write_file would clobber the host file because
        // tokio::fs::write follows symlinks.
        let tmp = workspace();
        // Plant a dangling symlink "innocent.txt" → "/etc/cron.d/imaginary"
        std::os::unix::fs::symlink(
            "/etc/cron.d/imaginary-payload-does-not-exist",
            tmp.path().join("innocent.txt"),
        )
        .unwrap();

        let err = canonicalize_in_workspace(tmp.path(), "innocent.txt")
            .expect_err("dangling symlink MUST be rejected");
        let msg = format!("{err:?}");
        assert!(msg.contains("symlink"), "msg: {msg}");
    }

    #[test]
    #[cfg(unix)]
    fn canonicalize_rejects_intermediate_symlink() {
        // sandbox can plant a symlink at an INTERMEDIATE directory
        // component (sub/foo.txt where `sub` is a symlink to /etc).
        // tokio::fs::write follows intermediate symlinks just like the
        // final component, so we reject any symlink along the path.
        let tmp = workspace();
        // Use a path that's almost certain to exist on every Linux
        // (/etc); the test only needs the symlink to be NOT-a-symlink-
        // at-the-target-side.
        std::os::unix::fs::symlink("/etc", tmp.path().join("sub")).unwrap();

        let err = canonicalize_in_workspace(tmp.path(), "sub/passwd")
            .expect_err("intermediate symlink MUST be rejected");
        let msg = format!("{err:?}");
        assert!(msg.contains("symlink"), "msg: {msg}");
    }

    #[test]
    #[cfg(unix)]
    fn canonicalize_rejects_symlink_to_existing_target() {
        // The original "symlink → existing host file" attack — also
        // rejected (now via the same symlink-component check rather
        // than the canonicalization heuristic).
        let tmp = workspace();
        std::os::unix::fs::symlink("/etc/passwd", tmp.path().join("foo")).unwrap();
        let err = canonicalize_in_workspace(tmp.path(), "foo")
            .expect_err("symlink to existing host file MUST be rejected");
        let msg = format!("{err:?}");
        assert!(msg.contains("symlink"), "msg: {msg}");
    }

    #[test]
    fn canonicalize_returns_resolved_path() {
        // Returned path should be inside the canonical workspace
        // (defense against TOCTOU: callers use this path for IO, not
        // the raw caller-supplied string).
        let tmp = workspace();
        let canon_workspace = std::fs::canonicalize(tmp.path()).unwrap();
        let p = canonicalize_in_workspace(tmp.path(), "foo.txt").unwrap();
        assert!(
            p.starts_with(&canon_workspace),
            "expected returned path {} to start with canonical workspace {}",
            p.display(),
            canon_workspace.display()
        );
    }

    #[tokio::test]
    async fn write_file_accepts_content_at_cap() {
        let tmp = workspace();
        let ctx = ctx_for(&tmp);
        let at_cap = "x".repeat(WRITE_FILE_MAX_BYTES);
        let res = write_file(&ctx, "ok.bin", &at_cap)
            .await
            .expect("at-cap must be accepted");
        assert_eq!(res["bytes_written"].as_u64().unwrap() as usize, WRITE_FILE_MAX_BYTES);
    }
}
