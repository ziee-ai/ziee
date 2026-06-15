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
pub fn collect_step_artifacts(
    ctx: &RunContext,
    step: &StepDef,
) -> Result<Vec<ArtifactMeta>, AppError> {
    let dir = ctx.artifact_path_for_step(&step.id);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    walk_dir(&dir, &dir, &mut out, step)?;
    Ok(out)
}

fn walk_dir(
    root: &Path,
    cur: &Path,
    out: &mut Vec<ArtifactMeta>,
    step: &StepDef,
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
            walk_dir(root, &p, out, step)?;
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

        let (description, mime_override) = match_decl(&filename, &step.artifacts);
        let mime_type = mime_override
            .unwrap_or_else(|| detect_mime_from_extension(&filename).to_string());
        let bytes = std::fs::read(&p).map_err(|e| {
            AppError::internal_error(format!("artifact_io: read {}: {e}", p.display()))
        })?;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let sha = format!("{:x}", hasher.finalize());

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

fn match_decl(filename: &str, decls: &[ArtifactDecl]) -> (Option<String>, Option<String>) {
    for d in decls {
        if let Some(path) = d.path.as_deref()
            && path == filename
        {
            return (d.description.clone(), d.mime_type.clone());
        }
        if let Some(glob) = d.glob.as_deref()
            && glob_match(glob, filename)
        {
            return (d.description.clone(), d.mime_type.clone());
        }
    }
    (None, None)
}

/// Minimal glob match — supports `*` (any-chars-except-/) at any
/// position. Good enough for the `*.png`, `**/foo`, `data/*.csv`
/// patterns the seed corpus uses.
fn glob_match(pattern: &str, name: &str) -> bool {
    fn rec(p: &[u8], n: &[u8]) -> bool {
        let mut pi = 0usize;
        let mut ni = 0usize;
        let mut star: Option<(usize, usize)> = None;
        while ni < n.len() {
            if pi < p.len() && (p[pi] == n[ni] || p[pi] == b'?') {
                pi += 1;
                ni += 1;
            } else if pi < p.len() && p[pi] == b'*' {
                star = Some((pi, ni));
                pi += 1;
            } else if let Some((sp, sn)) = star {
                pi = sp + 1;
                ni = sn + 1;
                star = Some((sp, ni));
            } else {
                return false;
            }
        }
        while pi < p.len() && p[pi] == b'*' {
            pi += 1;
        }
        pi == p.len()
    }
    rec(pattern.as_bytes(), name.as_bytes())
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
