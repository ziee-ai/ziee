// Shared export helper: map a target format to its MIME type, and render source
// bytes to that format via pandoc. Used by BOTH the per-file export endpoint
// (`file::handlers::export`) and the conversation export endpoint
// (`chat::core::export`), so the format matrix + temp-file dance live in one place.

use std::path::PathBuf;

use uuid::Uuid;

use crate::common::AppError;

use super::pandoc;

/// Map a supported export format to its MIME type, or `None` if unsupported.
pub fn export_mime(format: &str) -> Option<&'static str> {
    Some(match format {
        "md" => "text/markdown; charset=utf-8",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "pdf" => "application/pdf",
        "odt" => "application/vnd.oasis.opendocument.text",
        "rtf" => "application/rtf",
        "html" => "text/html; charset=utf-8",
        _ => return None,
    })
}

/// Render `input` (bytes carrying `input_ext`, e.g. `"md"`) to `format`.
/// `md` returns the input unchanged; every other format is produced by pandoc
/// (docx/odt/rtf/html native writers; pdf via the bundled typst engine). Writes
/// to a unique temp dir and always cleans it up (even on error).
pub async fn render_to_format(
    input: &[u8],
    input_ext: &str,
    format: &str,
) -> Result<Vec<u8>, AppError> {
    if format == "md" {
        return Ok(input.to_vec());
    }
    let ext = if input_ext.is_empty() { "md" } else { input_ext };
    let dir = std::env::temp_dir().join(format!("ziee-export-{}", Uuid::new_v4()));
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| AppError::internal_error(format!("export temp dir: {e}")))?;
    let in_path: PathBuf = dir.join(format!("input.{ext}"));
    let out_path: PathBuf = dir.join(format!("output.{format}"));
    let converted = async {
        tokio::fs::write(&in_path, input)
            .await
            .map_err(|e| AppError::internal_error(format!("export temp write: {e}")))?;
        pandoc::convert_to(&in_path, &out_path).await?;
        tokio::fs::read(&out_path)
            .await
            .map_err(|e| AppError::internal_error(format!("export read output: {e}")))
    }
    .await;
    let _ = tokio::fs::remove_dir_all(&dir).await;
    converted
}
