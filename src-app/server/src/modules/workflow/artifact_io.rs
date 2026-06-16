//! Per-step artifact collection (plan §4.7 Step artifacts).
//!
//! After a sandbox step completes, the runner walks
//! `<workspace>/<conv>/workflow/<run>/artifacts/<step_id>/` and
//! registers every file (or only the explicitly declared ones if
//! `artifacts: { collect: declared_only }`). For each file we:
//!   - re-check path safety (no symlink escape, no `..`),
//!   - match against the step's `artifacts:` declarations (path or
//!     glob),
//!   - synthesize description for unmatched files when collect = all,
//!   - detect mime from extension (override if author specified),
//!   - compute sha256,
//!   - cap-check (per-run cumulative artifact + output bytes ≤ 100
//!     MiB).
//!
//! Result is a `Vec<ArtifactMeta>` persisted into
//! `step_artifacts_json[step_id]`.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::common::AppError;
use crate::modules::workflow::types::{ArtifactMeta, RunContext};
use crate::modules::workflow::validate::{ArtifactDecl, StepDef};

pub const PER_RUN_ARTIFACT_CAP_BYTES: u64 = 100 * 1024 * 1024;
pub const PER_FILE_ARTIFACT_CAP_BYTES: u64 = 10 * 1024 * 1024;

/// Walk `artifacts/<step_id>/` and collect every regular file. Returns
/// the per-step `Vec<ArtifactMeta>`.
///
/// M3: `run_bytes_so_far` is the run's cumulative output+artifact byte
/// total BEFORE this step's artifacts. The walk checks the per-run cap
/// against the metadata size BEFORE reading a file's body into memory,
/// so a single huge artifact (or a long tail of them) is rejected
/// without ever buffering its bytes. Returns `Err` the moment the
/// running total would cross `PER_RUN_ARTIFACT_CAP_BYTES`.
pub fn collect_step_artifacts(
    ctx: &RunContext,
    step: &StepDef,
) -> Result<Vec<ArtifactMeta>, AppError> {
    let dir = ctx.artifact_path_for_step(&step.id);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    let mut running = ctx.total_output_bytes;
    walk_dir(&dir, &dir, &mut out, step, &mut running)?;
    Ok(out)
}

fn walk_dir(
    root: &Path,
    cur: &Path,
    out: &mut Vec<ArtifactMeta>,
    step: &StepDef,
    running: &mut u64,
) -> Result<(), AppError> {
    for entry in std::fs::read_dir(cur).map_err(|e| {
        AppError::internal_error(format!("artifact_io: read_dir {}: {e}", cur.display()))
    })? {
        let entry = entry
            .map_err(|e| AppError::internal_error(format!("artifact_io: entry: {e}")))?;
        let md = entry.metadata().map_err(|e| {
            AppError::internal_error(format!("artifact_io: stat: {e}"))
        })?;
        // Symlink rejection — defense in depth (bundle extractor already
        // rejects symlinks at the tar layer, and bwrap won't follow them
        // by default; but a sandbox script could have created one).
        let file_type = entry.file_type().map_err(|e| {
            AppError::internal_error(format!("artifact_io: file_type: {e}"))
        })?;
        if file_type.is_symlink() {
            tracing::warn!(path = %entry.path().display(), "artifact_io: skipping symlink");
            continue;
        }
        let p = entry.path();
        if md.is_dir() {
            walk_dir(root, &p, out, step, running)?;
            continue;
        }
        if !md.is_file() {
            continue;
        }
        let rel = p
            .strip_prefix(root)
            .map_err(|_| AppError::internal_error("artifact_io: strip_prefix failed"))?;
        let filename = rel.to_string_lossy().into_owned();

        if md.len() > PER_FILE_ARTIFACT_CAP_BYTES {
            tracing::warn!(
                path = %p.display(),
                size = md.len(),
                cap = PER_FILE_ARTIFACT_CAP_BYTES,
                "artifact_io: skipping oversize artifact"
            );
            continue;
        }

        // M3: per-run cap PRE-WRITE — check against the file's metadata
        // size before reading its body. Bail (run-fails) rather than
        // buffering bytes we'd only discard.
        if running.saturating_add(md.len()) > PER_RUN_ARTIFACT_CAP_BYTES {
            return Err(AppError::unprocessable_entity(
                "WORKFLOW_ARTIFACT_RUN_CAP",
                format!(
                    "per-run output+artifact byte cap {PER_RUN_ARTIFACT_CAP_BYTES} \
                     would be exceeded by artifact '{filename}' ({} bytes)",
                    md.len()
                ),
            ));
        }

        let (description, mime_override) = match_decl(&filename, &step.artifacts);
        let mime_type = mime_override
            .unwrap_or_else(|| detect_mime_from_extension(&filename).to_string());
        let bytes = std::fs::read(&p).map_err(|e| {
            AppError::internal_error(format!("artifact_io: read {}: {e}", p.display()))
        })?;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let sha = format!("{:x}", hasher.finalize());

        *running = running.saturating_add(md.len());
        out.push(ArtifactMeta {
            filename,
            host_path: p,
            size_bytes: md.len(),
            sha256: sha,
            mime_type,
            description,
        });
    }
    Ok(())
}

/// M2: match a filename against the step's artifact decls. Two passes so
/// an EXACT `path:` decl always wins over a broad `glob:` decl, even if
/// the glob decl appears first in the list (otherwise a `glob: "*"`
/// earlier in the list would steal a specific `path: "report.pdf"`
/// decl's metadata).
fn match_decl(filename: &str, decls: &[ArtifactDecl]) -> (Option<String>, Option<String>) {
    // Pass 1: exact path decls.
    for d in decls {
        if let Some(path) = d.path.as_deref()
            && path == filename
        {
            return (d.description.clone(), d.mime_type.clone());
        }
    }
    // Pass 2: glob decls.
    for d in decls {
        if let Some(glob) = d.glob.as_deref()
            && glob_match(glob, filename)
        {
            return (d.description.clone(), d.mime_type.clone());
        }
    }
    (None, None)
}

/// Minimal glob match. M2: a single `*` matches any run of chars EXCEPT
/// `/` (so `*.png` does NOT match `subdir/x.png`), and `**` matches
/// across `/` boundaries (so `**/foo` reaches into subdirs). `?` matches
/// a single non-`/` char. This makes glob semantics path-aware: a broad
/// `*.png` can't accidentally vacuum up files in nested artifact dirs.
fn glob_match(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let n: Vec<char> = name.chars().collect();
    fn rec(p: &[char], pi: usize, n: &[char], ni: usize) -> bool {
        if pi == p.len() {
            return ni == n.len();
        }
        match p[pi] {
            '*' => {
                // `**` — match across `/`.
                if pi + 1 < p.len() && p[pi + 1] == '*' {
                    // Collapse runs of `*`; `**` matches anything incl `/`.
                    let mut next = pi + 1;
                    while next < p.len() && p[next] == '*' {
                        next += 1;
                    }
                    for k in ni..=n.len() {
                        if rec(p, next, n, k) {
                            return true;
                        }
                    }
                    false
                } else {
                    // Single `*` — match any run NOT containing `/`.
                    let mut k = ni;
                    loop {
                        if rec(p, pi + 1, n, k) {
                            return true;
                        }
                        if k >= n.len() || n[k] == '/' {
                            return false;
                        }
                        k += 1;
                    }
                }
            }
            '?' => {
                if ni < n.len() && n[ni] != '/' {
                    rec(p, pi + 1, n, ni + 1)
                } else {
                    false
                }
            }
            c => {
                if ni < n.len() && n[ni] == c {
                    rec(p, pi + 1, n, ni + 1)
                } else {
                    false
                }
            }
        }
    }
    rec(&p, 0, &n, 0)
}

fn detect_mime_from_extension(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    let dot = match lower.rfind('.') {
        Some(i) => i,
        None => return "application/octet-stream",
    };
    match &lower[dot + 1..] {
        "md" | "markdown" => "text/markdown",
        "html" | "htm" => "text/html",
        "json" => "application/json",
        "csv" => "text/csv",
        "tsv" => "text/tab-separated-values",
        "yaml" | "yml" => "application/x-yaml",
        "txt" | "log" => "text/plain",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "tar" => "application/x-tar",
        "tgz" | "gz" => "application/gzip",
        _ => "application/octet-stream",
    }
}

/// Stream an artifact file by name. Used by the REST handler.
pub fn artifact_host_path(
    ctx: &RunContext,
    step_id: &str,
    filename: &str,
) -> Result<PathBuf, AppError> {
    // Path safety: filename must not escape.
    if filename.contains("..") || filename.starts_with('/') {
        return Err(AppError::bad_request(
            "ARTIFACT_PATH_INVALID",
            format!("artifact filename '{filename}' is not safe"),
        ));
    }
    Ok(ctx.artifact_path_for_step(step_id).join(filename))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_matches_star() {
        assert!(glob_match("*.png", "foo.png"));
        assert!(!glob_match("*.png", "foo.jpg"));
        assert!(glob_match("data/*.csv", "data/x.csv"));
        assert!(glob_match("*", "anything"));
        assert!(glob_match("foo.*", "foo.bar"));
    }

    #[test]
    fn glob_single_star_does_not_cross_slash() {
        // M2: `*` must NOT backtrack across `/` — a broad `*.png` can't
        // steal a file living in a subdir.
        assert!(!glob_match("*.png", "subdir/x.png"));
        assert!(!glob_match("*", "a/b"));
        assert!(!glob_match("data/*.csv", "data/sub/x.csv"));
        // `**` DOES cross `/`.
        assert!(glob_match("**/foo.png", "a/b/foo.png"));
        assert!(glob_match("**", "a/b/c"));
        assert!(glob_match("**/*.csv", "data/sub/x.csv"));
        // `?` is single-char, non-slash.
        assert!(glob_match("a?c", "abc"));
        assert!(!glob_match("a?c", "a/c"));
    }

    #[test]
    fn match_decl_prefers_exact_path_over_glob() {
        // M2: an exact `path:` decl wins even when a broad glob decl
        // appears EARLIER in the list.
        let decls = vec![
            ArtifactDecl {
                path: None,
                glob: Some("*".to_string()),
                description: Some("broad".to_string()),
                mime_type: None,
            },
            ArtifactDecl {
                path: Some("report.pdf".to_string()),
                glob: None,
                description: Some("the report".to_string()),
                mime_type: Some("application/pdf".to_string()),
            },
        ];
        let (desc, mime) = match_decl("report.pdf", &decls);
        assert_eq!(desc.as_deref(), Some("the report"));
        assert_eq!(mime.as_deref(), Some("application/pdf"));
    }

    #[test]
    fn mime_detection_smoke() {
        assert_eq!(detect_mime_from_extension("report.md"), "text/markdown");
        assert_eq!(detect_mime_from_extension("chart.png"), "image/png");
        assert_eq!(detect_mime_from_extension("README"), "application/octet-stream");
        assert_eq!(detect_mime_from_extension("Data.JSON"), "application/json");
    }

    #[test]
    fn path_safety_rejects_escape() {
        use crate::modules::workflow::types::RunContext;
        use std::path::PathBuf;
        use uuid::Uuid;
        let ctx = RunContext {
            run_id: Uuid::nil(),
            user_id: Uuid::nil(),
            conversation_id: None,
            workflow_id: Uuid::nil(),
            inputs: Default::default(),
            step_outputs: Default::default(),
            step_item_progress: Default::default(),
            extracted_path: PathBuf::from("/tmp"),
            sandbox_workspace: PathBuf::from("/tmp"),
            outputs_dir: PathBuf::from("/tmp"),
            artifacts_dir: PathBuf::from("/tmp"),
            inputs_dir: PathBuf::from("/tmp"),
            model_id: Uuid::nil(),
            model_name: "m".into(),
            model_max_tokens: 8192,
            sandbox_flavor: None,
            total_tokens: 0,
            total_output_bytes: 0,
            is_dev: false,
            mocks: std::collections::HashMap::new(),
            force_mocks: false,
        };
        assert!(artifact_host_path(&ctx, "s", "../../etc/passwd").is_err());
        assert!(artifact_host_path(&ctx, "s", "/abs").is_err());
        assert!(artifact_host_path(&ctx, "s", "ok.md").is_ok());
    }
}
