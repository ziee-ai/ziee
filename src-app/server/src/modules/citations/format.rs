//! Export + format a reference list.
//!
//! - CSL-JSON  → emitted directly (it's our storage format).
//! - BibTeX    → embedded pandoc `-f csljson -t bibtex` (titles double-braced
//!               to preserve capitalization — the `doi-to-ref.js` trick).
//! - RIS       → a small pure-Rust writer (pandoc has no RIS *writer*).
//! - Text      → pandoc `--citeproc` rendering in a CSL style (a named bundled
//!               style via `csl::style_path`, else pandoc's built-in default).

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::Value;

use crate::common::AppError;
use crate::modules::file::utils::pandoc::find_pandoc;

const PANDOC_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    CslJson,
    Bibtex,
    Ris,
    Text,
}

impl ExportFormat {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "csljson" | "csl-json" | "json" => Self::CslJson,
            "bibtex" | "bib" => Self::Bibtex,
            "ris" => Self::Ris,
            _ => Self::Text,
        }
    }
}

/// Render `items` (CSL-JSON values) in the requested format. `style_path` is an
/// optional path to a `.csl` file (used only by `Text`); `None` → pandoc's
/// built-in default style.
pub async fn export(
    items: Vec<Value>,
    format: ExportFormat,
    style_path: Option<PathBuf>,
) -> Result<String, AppError> {
    match format {
        ExportFormat::CslJson => serde_json::to_string_pretty(&items)
            .map_err(|e| AppError::internal_error(format!("csljson serialize: {e}"))),
        ExportFormat::Ris => Ok(to_ris(&items)),
        ExportFormat::Bibtex => {
            let json = serde_json::to_vec(&items)
                .map_err(|e| AppError::internal_error(format!("csljson serialize: {e}")))?;
            run_pandoc(
                &["-f".into(), "csljson".into(), "-t".into(), "bibtex".into()],
                json,
            )
            .await
        }
        ExportFormat::Text => render_text(items, style_path).await,
    }
}

/// Render the full bibliography as plain text via pandoc citeproc.
async fn render_text(items: Vec<Value>, style_path: Option<PathBuf>) -> Result<String, AppError> {
    // pandoc needs the bibliography as a FILE for --bibliography.
    let refs_path = write_temp_json(&items)?;
    let mut args: Vec<String> = vec![
        "--citeproc".into(),
        "-f".into(),
        "markdown".into(),
        "-t".into(),
        "plain".into(),
        "--bibliography".into(),
        refs_path.display().to_string(),
    ];
    if let Some(style) = &style_path {
        args.push("--csl".into());
        args.push(style.display().to_string());
    }
    // `nocite: '@*'` forces every reference into the rendered bibliography.
    let doc = "---\nnocite: '@*'\n---\n".as_bytes().to_vec();
    let out = run_pandoc(&args, doc).await;
    let _ = std::fs::remove_file(&refs_path);
    // Clean up the extracted CSL style temp file (csl::style_path leaves cleanup
    // to the caller — a unique file per call).
    if let Some(style) = &style_path {
        let _ = std::fs::remove_file(style);
    }
    out
}

/// Run pandoc with `args`, feeding `stdin`, returning stdout. Mirrors the
/// timeout/spawn_blocking hardening in `file/utils/pandoc.rs`.
///
/// stdin is written on a SEPARATE thread while the main thread drains stdout —
/// without that, a large input (e.g. the CSL-JSON of ~100 entries on the BibTeX
/// path, well over a pipe buffer) deadlocks: pandoc blocks writing stdout while
/// we block writing stdin. On timeout the child is killed so it isn't orphaned.
async fn run_pandoc(args: &[String], stdin: Vec<u8>) -> Result<String, AppError> {
    let pandoc = find_pandoc()?;
    let args = args.to_vec();
    let result = tokio::time::timeout(
        PANDOC_TIMEOUT,
        tokio::task::spawn_blocking(move || -> std::io::Result<std::process::Output> {
            let mut child = Command::new(&pandoc)
                .args(&args)
                .env("openout_any", "p")
                .env("openin_any", "p")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;
            // Write stdin on its own thread so we can drain stdout concurrently
            // (prevents the pipe-buffer deadlock); dropping `sin` closes stdin.
            if let Some(mut sin) = child.stdin.take() {
                std::thread::spawn(move || {
                    let _ = sin.write_all(&stdin);
                });
            }
            child.wait_with_output()
        }),
    )
    .await;

    let output = match result {
        Err(_) => return Err(AppError::internal_error("pandoc timed out after 60s")),
        Ok(Err(e)) => return Err(AppError::internal_error(format!("pandoc task panicked: {e}"))),
        Ok(Ok(Err(e))) => return Err(AppError::internal_error(format!("failed to run pandoc: {e}"))),
        Ok(Ok(Ok(o))) => o,
    };
    if !output.status.success() {
        return Err(AppError::internal_error(format!(
            "pandoc failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn write_temp_json(items: &[Value]) -> Result<PathBuf, AppError> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("ziee-citations-{}-{}.json", std::process::id(), n));
    let json = serde_json::to_vec(items)
        .map_err(|e| AppError::internal_error(format!("csljson serialize: {e}")))?;
    std::fs::write(&path, json)
        .map_err(|e| AppError::internal_error(format!("temp write: {e}")))?;
    Ok(path)
}

// ─────────────────────────── pure RIS writer ───────────────────────────

/// CSL `type` → RIS `TY`. A small, common mapping; defaults to JOUR.
fn ris_type(csl_type: &str) -> &'static str {
    match csl_type {
        "article-journal" | "article" => "JOUR",
        "book" => "BOOK",
        "chapter" => "CHAP",
        "paper-conference" => "CPAPER",
        "thesis" => "THES",
        "report" => "RPRT",
        "webpage" => "ELEC",
        "dataset" => "DATA",
        _ => "JOUR",
    }
}

/// RIS is line-oriented, so a value containing a newline would split into a
/// bogus second line (or inject a fake `ER  -` terminator). Collapse any
/// CR/LF in a field value to a space before emitting it.
fn ris_sanitize(s: &str) -> String {
    s.replace(['\r', '\n'], " ")
}

/// Minimal CSL-JSON → RIS. RIS is line-oriented `TAG  - value`; one record per
/// item, terminated by `ER  -`.
pub fn to_ris(items: &[Value]) -> String {
    let mut out = String::new();
    for it in items {
        let ty = it.get("type").and_then(|v| v.as_str()).unwrap_or("article-journal");
        out.push_str(&format!("TY  - {}\n", ris_type(ty)));
        if let Some(title) = it.get("title").and_then(|v| v.as_str()) {
            out.push_str(&format!("TI  - {}\n", ris_sanitize(title)));
        }
        if let Some(authors) = it.get("author").and_then(|v| v.as_array()) {
            for a in authors {
                let name = match (
                    a.get("family").and_then(|v| v.as_str()),
                    a.get("given").and_then(|v| v.as_str()),
                ) {
                    (Some(f), Some(g)) => format!("{f}, {g}"),
                    (Some(f), None) => f.to_string(),
                    _ => a
                        .get("literal")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                };
                if !name.is_empty() {
                    out.push_str(&format!("AU  - {}\n", ris_sanitize(&name)));
                }
            }
        }
        if let Some(year) = it
            .get("issued")
            .and_then(|i| i.get("date-parts"))
            .and_then(|d| d.as_array())
            .and_then(|a| a.first())
            .and_then(|p| p.as_array())
            .and_then(|p| p.first())
            .and_then(|y| y.as_i64())
        {
            out.push_str(&format!("PY  - {year}\n"));
        }
        if let Some(j) = it.get("container-title").and_then(|v| v.as_str()) {
            out.push_str(&format!("JO  - {}\n", ris_sanitize(j)));
        }
        if let Some(doi) = it.get("DOI").and_then(|v| v.as_str()) {
            out.push_str(&format!("DO  - {}\n", ris_sanitize(doi)));
        }
        if let Some(url) = it.get("URL").and_then(|v| v.as_str()) {
            out.push_str(&format!("UR  - {}\n", ris_sanitize(url)));
        }
        out.push_str("ER  - \n\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn export_format_parse() {
        assert_eq!(ExportFormat::parse("BibTeX"), ExportFormat::Bibtex);
        assert_eq!(ExportFormat::parse("ris"), ExportFormat::Ris);
        assert_eq!(ExportFormat::parse("csljson"), ExportFormat::CslJson);
        assert_eq!(ExportFormat::parse("anything"), ExportFormat::Text);
    }

    #[test]
    fn ris_writer_emits_expected_tags() {
        let items = vec![json!({
            "type": "article-journal",
            "title": "CRISPR interference in plants",
            "author": [{ "family": "Smith", "given": "J." }],
            "container-title": "Nature",
            "issued": { "date-parts": [[2021, 6, 1]] },
            "DOI": "10.1038/abc"
        })];
        let ris = to_ris(&items);
        assert!(ris.contains("TY  - JOUR"));
        assert!(ris.contains("TI  - CRISPR interference in plants"));
        assert!(ris.contains("AU  - Smith, J."));
        assert!(ris.contains("PY  - 2021"));
        assert!(ris.contains("JO  - Nature"));
        assert!(ris.contains("DO  - 10.1038/abc"));
        assert!(ris.trim_end().ends_with("ER  -"));
    }

    /// The Text export path shells out to `pandoc --csl <style>`. A nonexistent
    /// style file makes pandoc exit non-zero, exercising the subprocess
    /// error branch in `run_pandoc`/`export` (the previously-untested pandoc
    /// failure path). When pandoc itself is unavailable, `find_pandoc` also
    /// errors — either way `export` must surface an `Err`, never a silent
    /// empty string.
    #[tokio::test]
    async fn export_text_with_missing_csl_style_surfaces_error() {
        let items = vec![json!({
            "type": "article-journal",
            "id": "a",
            "title": "X"
        })];
        let bogus = std::path::PathBuf::from("/nonexistent/ziee-test-style-does-not-exist.csl");
        let res = export(items, ExportFormat::Text, Some(bogus)).await;
        assert!(
            res.is_err(),
            "a missing CSL style must surface a pandoc error, not an empty Ok"
        );
    }

    /// The CslJson + Ris export branches are pure-Rust (no pandoc) and must
    /// round-trip deterministically through the public `export` dispatch.
    #[tokio::test]
    async fn export_csljson_and_ris_need_no_pandoc() {
        let items = vec![json!({
            "type": "article-journal",
            "title": "CRISPR interference in plants",
            "DOI": "10.1038/abc"
        })];
        let cj = export(items.clone(), ExportFormat::CslJson, None)
            .await
            .expect("csljson export is infallible for valid items");
        assert!(cj.contains("CRISPR interference in plants"));

        let ris = export(items, ExportFormat::Ris, None)
            .await
            .expect("ris export is infallible");
        assert!(ris.contains("TY  - JOUR"));
        assert!(ris.contains("DO  - 10.1038/abc"));
    }
}
