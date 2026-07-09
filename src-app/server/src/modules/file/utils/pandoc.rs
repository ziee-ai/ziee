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

/// Convert a document to `output_path`, inferring the pandoc writer from the
/// output file extension. PDF routes through the bundled typst engine (see
/// `convert_to_pdf`); `docx`/`odt`/`rtf`/`html` are native pandoc writers that
/// need no engine. Same 60s wall-clock timeout hardening as `convert_to_pdf`.
pub async fn convert_to(input_path: &PathBuf, output_path: &PathBuf) -> Result<(), AppError> {
    // PDF needs the bundled typst engine — delegate to the hardened PDF path.
    let is_pdf = output_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false);
    if is_pdf {
        return convert_to_pdf(input_path, output_path).await;
    }

    let pandoc_path = find_pandoc()?;
    let input_path = input_path.clone();
    let output_path = output_path.clone();

    let result = tokio::time::timeout(
        PANDOC_TIMEOUT,
        tokio::task::spawn_blocking(move || {
            Command::new(&pandoc_path)
                .arg(&input_path)
                .arg("-o")
                .arg(&output_path)
                // Harmless for the native writers (docx/odt/rtf/html); kept for
                // defense-in-depth parity with the PDF path.
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
            "Pandoc conversion failed: {}",
            stderr
        )));
    }

    Ok(())
}

/// Convert document to PDF using Pandoc
pub async fn convert_to_pdf(
    input_path: &PathBuf,
    output_path: &PathBuf,
) -> Result<(), AppError> {
    let pandoc_path = find_pandoc()?;

    // ENGINE: use typst (not pdflatex / xelatex). pdflatex's default
    // 8-bit encoding chokes on arbitrary Unicode the moment a doc
    // contains common symbols like ≥, ≤, →, π, etc. — it raises
    // `LaTeX Error: Unicode character … not set up for use with
    // LaTeX` and produces no PDF. Real-world DOCX / RTF / ODT
    // uploads hit this constantly (any technical / scientific text).
    //
    // typst reads UTF-8 natively, ships as a single static binary
    // we embed via include_bytes! (build_helper/typst.rs +
    // utils/embedded.rs), and is supported by pandoc as a first-
    // class PDF engine since pandoc 3.1.7. Picking typst over
    // xelatex avoids bundling the entire TeX Live distribution —
    // critical for the self-contained-binary distribution model.
    //
    // We pass `--pdf-engine` the FULL PATH to the extracted typst
    // binary so pandoc doesn't need it on PATH at runtime.
    //
    // The shell-escape / openout_any environment hardening below is
    // a no-op for typst (typst has no equivalent of LaTeX's
    // `\write18`), but it's kept in place for defense-in-depth in
    // case a future operator switches the engine back to xelatex
    // without re-reading this comment.
    //
    // SECURITY: wall-clock timeout via tokio::time::timeout around
    // spawn_blocking. The Command's child is killed via Drop on the
    // JoinHandle when the timeout fires. Closes 05-file F-09 (Medium).
    let typst_path = super::embedded::get_typst_path()?.clone();
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
                .arg(format!("--pdf-engine={}", typst_path.display()))
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
