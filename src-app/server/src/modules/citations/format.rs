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
/// Coerce a stored CSL-JSON record into something a STRICT CSL reader accepts.
///
/// doi.org content negotiation is the documented way to get CSL-JSON, but the
/// records Crossref serves through it are not strictly CSL: `ISSN`/`ISBN` come
/// back as arrays, and extra Crossref-native keys ride along (`link`, `license`,
/// `assertion`, `subject`, `relation`, `reference`, `content-domain`). Pandoc is
/// a strict reader and rejects the whole file:
///
///   Error in $[0]: expected String or Number, but encountered Array
///
/// which surfaced as a hard 500 on GET /api/citations/export for any library
/// containing a single Crossref-sourced entry — the export was unusable, not
/// degraded. Found by the live UI explorer, reproduced from the stored record.
///
/// Sanitising here rather than at ingest is deliberate: the stored blob stays a
/// faithful copy of what the provider returned (it is the source of truth, and
/// re-fetching is expensive), and every export path shares one normalisation.
fn sanitize_csl(item: &Value) -> Value {
    // Keys that are Crossref-native and have no CSL meaning. Passing them through
    // is what makes a strict reader fail; none carry citation semantics.
    const DROP: &[&str] = &[
        "link", "license", "assertion", "relation", "reference", "content-domain",
        "journal-issue", "resource", "update-policy", "subject", "article-number",
        "alternative-id", "prefix", "member", "score", "deposited", "indexed",
        "reference-count", "references-count", "is-referenced-by-count",
    ];
    // CSL declares these as a single value; Crossref sends arrays. Take the first.
    const FIRST_OF_ARRAY: &[&str] = &["ISSN", "ISBN", "container-title", "title", "short-title"];

    let Some(obj) = item.as_object() else { return item.clone() };
    let mut out = serde_json::Map::new();
    for (k, v) in obj {
        if DROP.contains(&k.as_str()) {
            continue;
        }
        if FIRST_OF_ARRAY.contains(&k.as_str()) {
            if let Some(arr) = v.as_array() {
                // An empty array means the field is simply absent, not empty-string.
                match arr.first() {
                    Some(first) => {
                        out.insert(k.clone(), first.clone());
                    }
                    None => {}
                }
                continue;
            }
        }
        // Any other bare array/object where CSL wants a scalar would fail the same
        // way. Name-ish and date-ish fields are legitimately structured, so they
        // are left alone; everything else that is an array of scalars collapses.
        if let Some(arr) = v.as_array() {
            let structured = matches!(
                k.as_str(),
                "author" | "editor" | "translator" | "container-author" | "original-author"
                    | "recipient" | "interviewer" | "composer" | "director" | "editorial-director"
                    | "illustrator" | "reviewed-author" | "issued" | "accessed" | "event-date"
                    | "submitted" | "original-date" | "categories"
            );
            if !structured {
                if arr.iter().all(|x| x.is_string() || x.is_number()) {
                    match arr.first() {
                        Some(first) => {
                            out.insert(k.clone(), first.clone());
                        }
                        None => {}
                    }
                } // an array of objects in a non-structured field: drop it
                continue;
            }
        }
        out.insert(k.clone(), v.clone());
    }
    Value::Object(out)
}

pub async fn export(
    items: Vec<Value>,
    format: ExportFormat,
    style_path: Option<PathBuf>,
) -> Result<String, AppError> {
    // Every format goes through the same normalisation, so a record that exports
    // as CSL-JSON cannot fail as BibTeX or text.
    let items: Vec<Value> = items.iter().map(sanitize_csl).collect();
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

    /// A field value containing CR/LF must NOT corrupt the line-oriented RIS
    /// structure: a newline in the title could otherwise split into a bogus
    /// second line or inject a premature `ER  -` terminator (record-smuggling).
    /// `ris_sanitize` collapses CR/LF to a space; assert the value stays on its
    /// own single tag line and the record carries exactly one terminator.
    #[test]
    fn ris_writer_sanitizes_newlines_and_resists_injection() {
        let items = vec![json!({
            "type": "article-journal",
            // Injected CR/LF + a forged terminator + a forged second record.
            "title": "Sneaky\r\nER  - \r\nTY  - BOOK\nTI  - Forged",
            "author": [{ "family": "Mc\nEvil", "given": "A." }],
            "DOI": "10.1/x",
        })];
        let ris = to_ris(&items);

        // The whole forged title lands on ONE `TI  -` line: no newline survives
        // inside the value, so the forged `ER  -`/`TY  -` fragments stay inert
        // text rather than becoming real RIS lines. (Exact run-length of the
        // collapsed whitespace is unimportant — the single-line invariant is.)
        let ti_line = ris
            .lines()
            .find(|l| l.starts_with("TI  - "))
            .expect("a TI line");
        assert!(
            ti_line.contains("Sneaky") && ti_line.contains("Forged"),
            "the entire CR/LF title must collapse onto one TI line, got: {ti_line:?}"
        );
        // The author CR/LF is likewise collapsed onto a single AU line.
        let au_line = ris
            .lines()
            .find(|l| l.starts_with("AU  - "))
            .expect("an AU line");
        assert!(
            au_line.contains("Mc") && au_line.contains("Evil"),
            "author CR/LF must collapse onto one AU line, got: {au_line:?}"
        );
        // Exactly one REAL record terminator line — the forged `ER  - ` did not
        // smuggle a premature record boundary.
        assert_eq!(
            ris.lines().filter(|l| *l == "ER  - ").count(),
            1,
            "exactly one record terminator; forged ER must not split the record:\n{ris}"
        );
        // Exactly one REAL record-type line — the forged `TY  - BOOK` is inert.
        assert_eq!(
            ris.lines().filter(|l| l.starts_with("TY  - ")).count(),
            1,
            "exactly one TY line; forged TY must not become a real line:\n{ris}"
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

#[cfg(test)]
mod sanitize_tests {
    use super::*;
    use serde_json::json;

    /// The exact shape that produced a 500 in the live rig: a Crossref record
    /// served through doi.org's CSL-JSON negotiation, with ISSN as an array and
    /// Crossref-native keys attached.
    fn crossref_shaped() -> Value {
        json!({
            "id": "cf7199ff", "type": "article-journal",
            "title": "Highly accurate protein structure prediction",
            "ISSN": ["0028-0836", "1476-4687"],
            "link": [{"URL": "https://www.nature.com/x.pdf", "content-type": "application/pdf"}],
            "license": [{"URL": "https://creativecommons.org/licenses/by/4.0"}],
            "subject": [], "subtitle": [],
            "assertion": [{"name": "received", "group": {"name": "ArticleHistory"}}],
            "reference-count": 1234,
            "author": [{"family": "Jumper", "given": "John"}],
            "issued": {"date-parts": [[2021, 7, 15]]},
            "container-title": ["Nature"]
        })
    }

    #[test]
    fn strips_crossref_extras_and_flattens_scalar_arrays() {
        let out = sanitize_csl(&crossref_shaped());
        let o = out.as_object().unwrap();
        // scalar-in-CSL fields become scalars
        assert_eq!(o.get("ISSN").unwrap(), "0028-0836");
        assert_eq!(o.get("container-title").unwrap(), "Nature");
        // Crossref-native keys are gone
        for k in ["link", "license", "assertion", "subject", "reference-count"] {
            assert!(!o.contains_key(k), "{k} should have been dropped");
        }
        // an empty array is absence, not empty string
        assert!(!o.contains_key("subtitle"));
        // structured CSL fields survive untouched
        assert!(o.get("author").unwrap().is_array());
        assert!(o.get("issued").unwrap().is_object());
        assert_eq!(o.get("type").unwrap(), "article-journal");
    }

    #[test]
    fn no_bare_arrays_remain_where_csl_wants_a_scalar() {
        let out = sanitize_csl(&crossref_shaped());
        for (k, v) in out.as_object().unwrap() {
            if v.is_array() {
                assert!(
                    matches!(k.as_str(), "author" | "editor" | "translator" | "categories"),
                    "unexpected array left in {k}, a strict CSL reader would reject it"
                );
            }
        }
    }

    #[test]
    fn already_clean_csl_is_unchanged() {
        let clean = json!({"id": "x", "type": "book", "title": "T", "ISSN": "1234-5678"});
        assert_eq!(sanitize_csl(&clean), clean);
    }
}
