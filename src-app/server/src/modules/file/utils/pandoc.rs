// Pandoc utility for runtime usage

use crate::common::AppError;
use std::path::PathBuf;
use std::process::Command;

/// Find Pandoc binary path
pub fn find_pandoc() -> Result<PathBuf, AppError> {
    // Try embedded binary first (extracted to app_data_dir/bin/)
    match super::embedded::get_pandoc_path() {
        Ok(path) => {
            tracing::debug!("Using embedded Pandoc at {:?}", path);
            return Ok(path.clone());
        }
        Err(e) => {
            tracing::warn!("Failed to get embedded Pandoc: {}, trying system", e);
        }
    }

    // Fall back to system pandoc
    match which::which("pandoc") {
        Ok(path) => {
            tracing::debug!("Found system Pandoc at {:?}", path);
            Ok(path)
        }
        Err(_) => Err(AppError::internal_error(
            "Pandoc not found. Embedded binary failed to extract and system pandoc not installed.",
        )),
    }
}

/// Convert document to PDF using Pandoc
pub async fn convert_to_pdf(
    input_path: &PathBuf,
    output_path: &PathBuf,
) -> Result<(), AppError> {
    let pandoc_path = find_pandoc()?;

    let output = Command::new(pandoc_path)
        .arg(input_path)
        .arg("-o")
        .arg(output_path)
        .arg("--pdf-engine=pdflatex") // or use weasyprint if available
        .output()
        .map_err(|e| AppError::internal_error(format!("Failed to run Pandoc: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::internal_error(format!(
            "Pandoc PDF conversion failed: {}",
            stderr
        )));
    }

    Ok(())
}
