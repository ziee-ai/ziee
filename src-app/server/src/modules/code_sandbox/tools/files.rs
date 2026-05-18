use std::path::PathBuf;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::code_sandbox::types::SandboxContext;
use crate::modules::code_sandbox::models::ConversationFile;
use crate::modules::code_sandbox::sandbox::mime_to_ext;

fn workspace_path(data_dir: &PathBuf, conversation_id: Uuid) -> PathBuf {
    data_dir.join("sandboxes").join(conversation_id.to_string())
}

/// Read a file by filename.
///
/// Step 1: Try the originals storage (user-attached files).
/// Step 2: Fall back to the workspace directory (LLM-written files).
pub async fn read_file(
    data_dir: &PathBuf,
    conversation_id: Uuid,
    files: &[ConversationFile],
    filename: &str,
    start_line: Option<usize>,
    end_line: Option<usize>,
) -> Result<String, AppError> {
    let content = if let Some(conv_file) = files.iter().find(|f| f.filename == filename) {
        // Originals first: read the user-attached file from storage
        let ext = conv_file
            .mime_type
            .as_deref()
            .and_then(|m| mime_to_ext(m))
            .unwrap_or("bin");

        let originals_path = data_dir
            .join("files")
            .join("originals")
            .join(conv_file.user_id.to_string())
            .join(format!("{}.{}", conv_file.file_id, ext));

        tokio::fs::read_to_string(&originals_path).await.map_err(|e| {
            AppError::internal_error(format!("Failed to read original file: {}", e))
        })?
    } else {
        // Fallback: workspace directory (LLM-written files)
        let workspace = workspace_path(data_dir, conversation_id);
        let workspace_file = workspace.join(filename);

        if !workspace_file.exists() {
            return Err(AppError::not_found("File not found in conversation attachments or workspace"));
        }

        tokio::fs::read_to_string(&workspace_file).await.map_err(|e| {
            AppError::internal_error(format!("Failed to read workspace file: {}", e))
        })?
    };

    // Return content with 1-indexed line numbers so the LLM can use them directly with edit_file
    let lines: Vec<&str> = content.lines().collect();
    let start = start_line.map(|s| s.saturating_sub(1)).unwrap_or(0);
    let end = end_line.map(|e| e.min(lines.len())).unwrap_or(lines.len());

    let numbered: String = lines[start..end]
        .iter()
        .enumerate()
        .map(|(i, line)| format!("{}: {}", start + i + 1, line))
        .collect::<Vec<_>>()
        .join("\n");

    Ok(numbered)
}

/// Write a file to the workspace.
pub async fn write_file(
    data_dir: &PathBuf,
    conversation_id: Uuid,
    filename: &str,
    content: &str,
) -> Result<(), AppError> {
    let workspace = workspace_path(data_dir, conversation_id);
    tokio::fs::create_dir_all(&workspace).await.map_err(|e| {
        AppError::internal_error(format!("Failed to create workspace: {}", e))
    })?;
    tokio::fs::write(workspace.join(filename), content)
        .await
        .map_err(|e| AppError::internal_error(format!("Failed to write file: {}", e)))
}

/// Edit a file by replacing a range of lines with new content (1-indexed, inclusive).
///
/// Special case: start_line == line_count + 1 appends new_content after the last line.
/// Pass empty new_content to delete lines without replacement.
///
/// Source priority: workspace first (preserves prior edits), then originals (user-attached files).
pub async fn edit_file(
    data_dir: &PathBuf,
    conversation_id: Uuid,
    files: &[ConversationFile],
    filename: &str,
    start_line: usize,
    end_line: usize,
    new_content: &str,
) -> Result<(), AppError> {
    let workspace = workspace_path(data_dir, conversation_id);
    let path = workspace.join(filename);

    let content = if path.exists() {
        // Workspace version: LLM-created or already-edited copy of an attachment
        tokio::fs::read_to_string(&path).await.map_err(|e| {
            AppError::internal_error(format!("Failed to read file for editing: {}", e))
        })?
    } else if let Some(conv_file) = files.iter().find(|f| f.filename == filename) {
        // First edit of a user-attached file — read from originals
        let ext = conv_file.mime_type.as_deref().and_then(|m| mime_to_ext(m)).unwrap_or("bin");
        let originals_path = data_dir
            .join("files")
            .join("originals")
            .join(conv_file.user_id.to_string())
            .join(format!("{}.{}", conv_file.file_id, ext));
        tokio::fs::read_to_string(&originals_path).await.map_err(|e| {
            AppError::internal_error(format!("Failed to read original file for editing: {}", e))
        })?
    } else {
        return Err(AppError::not_found("File not found in workspace or conversation attachments"));
    };

    if start_line < 1 {
        return Err(AppError::bad_request("invalid_range", "start_line must be >= 1"));
    }

    let lines: Vec<&str> = content.lines().collect();
    let has_trailing_newline = content.ends_with('\n');

    tokio::fs::create_dir_all(&workspace).await.map_err(|e| {
        AppError::internal_error(format!("Failed to create workspace: {}", e))
    })?;

    // Append mode: start_line == line_count + 1 means "add after last line"
    if start_line == lines.len() + 1 {
        if new_content.is_empty() {
            return Ok(()); // nothing to append
        }
        // Ensure exactly one \n separator:
        // - content ends with \n: use it directly (sep = "")
        // - content has no trailing \n: add one (sep = "\n")
        let sep = if content.is_empty() || has_trailing_newline { "" } else { "\n" };
        let updated = format!("{}{}{}", content, sep, new_content);
        return tokio::fs::write(&path, updated)
            .await
            .map_err(|e| AppError::internal_error(format!("Failed to write file: {}", e)));
    }

    if start_line > lines.len() {
        return Err(AppError::bad_request(
            "invalid_range",
            format!(
                "start_line {} exceeds file length {} (use {} to append after last line)",
                start_line,
                lines.len(),
                lines.len() + 1
            ),
        ));
    }
    if end_line < start_line {
        return Err(AppError::bad_request("invalid_range", "end_line must be >= start_line"));
    }
    let end = end_line.min(lines.len()); // clamp to file length

    let mut result: Vec<&str> = lines[..start_line - 1].to_vec();
    result.extend(new_content.lines());
    result.extend(lines[end..].iter().copied());

    // join("\n") puts \n BETWEEN elements only (no trailing \n).
    // Add "\n" at end only when original had one — preserves file's newline convention.
    let updated = if has_trailing_newline {
        result.join("\n") + "\n"
    } else {
        result.join("\n")
    };

    tokio::fs::write(&path, updated)
        .await
        .map_err(|e| AppError::internal_error(format!("Failed to write edited file: {}", e)))
}

#[derive(serde::Serialize)]
pub struct FileEntry {
    pub name: String,
    pub size: u64,
    pub is_file: bool,
}

/// List files in the workspace root (no recursion).
/// Hidden files (prefixed with '.sandbox_') used internally are excluded.
pub async fn list_files(
    data_dir: &PathBuf,
    conversation_id: Uuid,
) -> Result<Vec<FileEntry>, AppError> {
    let workspace = workspace_path(data_dir, conversation_id);
    tokio::fs::create_dir_all(&workspace).await.map_err(|e| {
        AppError::internal_error(format!("Failed to create workspace: {}", e))
    })?;

    let mut entries = Vec::new();
    let mut dir = tokio::fs::read_dir(&workspace).await.map_err(|e| {
        AppError::internal_error(format!("Failed to read workspace: {}", e))
    })?;

    while let Some(entry) = dir.next_entry().await.map_err(|e| {
        AppError::internal_error(format!("Failed to read directory entry: {}", e))
    })? {
        let name = entry.file_name().to_string_lossy().into_owned();
        // Skip internal helper files
        if name.starts_with(".sandbox_") {
            continue;
        }
        let meta = entry.metadata().await.unwrap_or_else(|_| {
            // If we can't get metadata, create a dummy entry
            unreachable!()
        });
        entries.push(FileEntry {
            name,
            size: meta.len(),
            is_file: meta.is_file(),
        });
    }

    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

/// Return a standard MCP resource_link for a file.
///
/// Checks user-attached (originals) files first — returns `is_saved: true` with a
/// short-lived download-with-token URL so the MCP client can skip the full
/// fetch-and-save pipeline.  Falls back to the workspace for LLM-generated files,
/// returning `is_saved: false` so the normal processing pipeline runs.
pub async fn get_resource_link(
    ctx: &SandboxContext,
    files: &[ConversationFile],
    filename: &str,
    save_as: Option<&str>,
) -> Result<Value, AppError> {
    let conversation_id = ctx.conversation_id.ok_or_else(|| {
        AppError::bad_request("missing_conversation", "Tool calls require a conversation context")
    })?;

    let display_name = save_as.unwrap_or(filename);
    let ext = std::path::Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin");
    let mime = mime_guess::from_ext(ext).first().map(|m| m.to_string());

    // Check originals first (user-attached files)
    if let Some(conv_file) = files.iter().find(|f| f.filename == filename) {
        use jsonwebtoken::{encode, EncodingKey, Header as JwtHeader};
        use crate::modules::file::types::DownloadTokenClaims;

        let now = chrono::Utc::now().timestamp() as usize;
        let claims = DownloadTokenClaims {
            file_id: conv_file.file_id.to_string(),
            user_id: conv_file.user_id.to_string(),
            exp: now + 3600,
            iat: now,
        };
        let token = encode(
            &JwtHeader::default(),
            &claims,
            &EncodingKey::from_secret(ctx.jwt_secret.as_bytes()),
        )
        .map_err(|e| AppError::internal_error(format!("Failed to generate download token: {}", e)))?;

        let mut uri_url = url::Url::parse(&ctx.base_url)
            .map_err(|e| AppError::internal_error(format!("Invalid base_url: {}", e)))?;
        uri_url.set_path(&format!("/api/files/{}/download-with-token", conv_file.file_id));
        uri_url.query_pairs_mut().append_pair("token", &token);

        return Ok(json!({
            "content": [{
                "type": "resource_link",
                "uri": uri_url.to_string(),
                "name": display_name,
                "mimeType": mime,
                "is_saved": true,
            }],
            "isError": false
        }));
    }

    // Workspace fallback (LLM-generated files)
    let workspace = workspace_path(&ctx.data_dir, conversation_id);
    let path = workspace.join(filename);
    if !path.exists() {
        return Err(AppError::not_found("File not found in conversation attachments or workspace"));
    }

    let mut uri_url = url::Url::parse(&ctx.base_url)
        .map_err(|e| AppError::internal_error(format!("Invalid base_url: {}", e)))?;
    uri_url.set_path("/api/code-sandbox/file/download");
    uri_url.query_pairs_mut().append_pair("filename", filename);

    Ok(json!({
        "content": [{
            "type": "resource_link",
            "uri": uri_url.to_string(),
            "name": display_name,
            "mimeType": mime,
            "is_saved": false,
        }],
        "isError": false
    }))
}

