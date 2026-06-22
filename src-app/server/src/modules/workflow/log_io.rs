//! Per-step diagnostic log capture (plan §4.7 log resources).
//!
//! What's captured (level-gated via `step.log: off | stderr | full`):
//! - rendered prompt (llm / llm_map) → `logs/<step_id>/prompt`
//! - raw LLM response BEFORE parse (llm / llm_map) → `logs/<step_id>/raw_output`
//! - sandbox stderr (`log: stderr | full`) → `logs/<step_id>/stderr`
//! - per-item llm_map input + raw_output + error → `logs/<step_id>/items/<N>.json`
//! - step trace (always when run; timing + tokens + on_error decisions)
//!   → `logs/<step_id>/trace.json`
//!
//! Capture writes to `<workspace>/<conv>/workflow/<run>/logs/<step_id>/<kind>`.
//! M1: the captured body is ALWAYS written to disk (the file is the
//! source of truth, streamed back via `log_stream`); the per-run
//! `step_logs_json` metadata stores only `{path, size_bytes, preview}`
//! where `preview` is the first 500 chars. There is no inline-vs-spill
//! threshold — the previously-declared (and unused) 64 KiB
//! `SPILL_THRESHOLD_BYTES` has been removed to match actual behavior.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::common::AppError;
use crate::modules::workflow::types::RunContext;
use crate::modules::workflow::validate::LogCapture;

/// Per-log body cap (chars) stored in `step_logs_json` for durability.
pub const LOG_BODY_CAP_CHARS: usize = 256 * 1024;

/// E7: per-RUN aggregate cap on durable log-body chars. The per-log cap above
/// bounds each body; this bounds the whole run (≤ ~16 bodies at the per-log
/// max) so a many-step debug-capture run can't bloat `step_logs_json`. Beyond
/// it, a marker is stored instead of the body.
pub const RUN_LOG_BODY_CAP_CHARS: usize = 4 * 1024 * 1024;

/// Cap a body for durable storage honoring BOTH the per-log cap and the
/// per-run aggregate budget (E7). Returns a marker once the run budget is
/// spent; otherwise the per-log-capped body, charging its length to the run.
fn cap_body_run(ctx: &RunContext, body: &str) -> Option<String> {
    use std::sync::atomic::Ordering;
    if ctx.total_log_bytes.load(Ordering::Relaxed) as usize >= RUN_LOG_BODY_CAP_CHARS {
        return Some("…[run log cap reached]".to_string());
    }
    let capped = cap_body(body);
    ctx.total_log_bytes
        .fetch_add(capped.chars().count() as u64, Ordering::Relaxed);
    Some(capped)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntryMeta {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub preview: String,
    /// A7: full captured body (capped at `LOG_BODY_CAP_CHARS`), persisted in
    /// `step_logs_json` so logs survive the staging-dir GC. `None` for legacy
    /// rows; a truncated body ends with a marker.
    #[serde(default)]
    pub body: Option<String>,
}

/// Cap a log body for durable storage in `step_logs_json`.
fn cap_body(body: &str) -> String {
    if body.chars().count() > LOG_BODY_CAP_CHARS {
        let mut s: String = body.chars().take(LOG_BODY_CAP_CHARS).collect();
        s.push_str("\n…[truncated]");
        s
    } else {
        body.to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StepTrace {
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub ms_elapsed: u64,
    pub tokens_used: u64,
    pub attempts: u32,
    pub on_error: Option<String>,
}

pub fn step_log_dir(ctx: &RunContext, step_id: &str) -> PathBuf {
    ctx.sandbox_workspace.join("logs").join(step_id)
}

pub async fn ensure_dir(dir: &Path) -> Result<(), AppError> {
    tokio::fs::create_dir_all(dir).await.map_err(|e| {
        AppError::internal_error(format!("log_io: mkdir {}: {e}", dir.display()))
    })
}

/// Write a text-shaped log (prompt / raw_output / stderr) and return
/// its meta. Always writes to disk; returns the meta for the DB row.
pub async fn write_text_log(
    ctx: &RunContext,
    step_id: &str,
    kind: &str,
    body: &str,
    log_level: LogCapture,
) -> Result<Option<LogEntryMeta>, AppError> {
    // A7b: the per-run debug toggle forces full capture regardless of `log:`.
    let level = if ctx.force_log_capture {
        LogCapture::Full
    } else {
        log_level
    };
    if !should_capture(kind, level) {
        return Ok(None);
    }
    let dir = step_log_dir(ctx, step_id);
    ensure_dir(&dir).await?;
    let dest = dir.join(kind);
    tokio::fs::write(&dest, body.as_bytes()).await.map_err(|e| {
        AppError::internal_error(format!("log_io: write {}: {e}", dest.display()))
    })?;
    let size = body.len() as u64;
    let preview = body.chars().take(500).collect::<String>();
    Ok(Some(LogEntryMeta {
        path: dest,
        size_bytes: size,
        preview,
        body: cap_body_run(ctx, body),
    }))
}

/// Per-item log for `llm_map`. Always captured at `log: full`; omitted
/// otherwise. Stored as JSON for round-trip with the FE renderer.
pub async fn write_item_log(
    ctx: &RunContext,
    step_id: &str,
    item_index: usize,
    record: &serde_json::Value,
    log_level: LogCapture,
) -> Result<Option<LogEntryMeta>, AppError> {
    if !(ctx.force_log_capture || log_level == LogCapture::Full) {
        return Ok(None);
    }
    let dir = step_log_dir(ctx, step_id).join("items");
    ensure_dir(&dir).await?;
    let dest = dir.join(format!("{item_index}.json"));
    let bytes = serde_json::to_vec_pretty(record)
        .map_err(|e| AppError::internal_error(format!("log_io: serialize item: {e}")))?;
    tokio::fs::write(&dest, &bytes).await.map_err(|e| {
        AppError::internal_error(format!("log_io: write {}: {e}", dest.display()))
    })?;
    let body_str = String::from_utf8_lossy(&bytes).into_owned();
    let preview = body_str.chars().take(500).collect::<String>();
    Ok(Some(LogEntryMeta {
        path: dest,
        size_bytes: bytes.len() as u64,
        preview,
        body: cap_body_run(ctx, &body_str),
    }))
}

/// Always-on per-step trace (timing + tokens). Persists to
/// `logs/<step_id>/trace.json`.
pub async fn write_trace(
    ctx: &RunContext,
    step_id: &str,
    trace: &StepTrace,
) -> Result<LogEntryMeta, AppError> {
    let dir = step_log_dir(ctx, step_id);
    ensure_dir(&dir).await?;
    let dest = dir.join("trace.json");
    let bytes = serde_json::to_vec_pretty(trace)
        .map_err(|e| AppError::internal_error(format!("log_io: serialize trace: {e}")))?;
    tokio::fs::write(&dest, &bytes).await.map_err(|e| {
        AppError::internal_error(format!("log_io: write {}: {e}", dest.display()))
    })?;
    let body_str = String::from_utf8_lossy(&bytes).into_owned();
    let preview = body_str.chars().take(500).collect::<String>();
    Ok(LogEntryMeta {
        path: dest,
        size_bytes: bytes.len() as u64,
        preview,
        body: Some(cap_body(&body_str)),
    })
}

/// Should this `(kind, level)` pair be captured?
fn should_capture(kind: &str, level: LogCapture) -> bool {
    match (kind, level) {
        ("stderr", LogCapture::Stderr) => true,
        ("stderr", LogCapture::Full) => true,
        ("prompt", LogCapture::Full) => true,
        ("raw_output", LogCapture::Full) => true,
        // Trace is always captured via write_trace, not write_text_log.
        // Anything else: only full.
        (_, LogCapture::Full) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use uuid::Uuid;

    fn fake_ctx(ws: PathBuf) -> RunContext {
        RunContext {
            run_id: Uuid::nil(),
            user_id: Uuid::nil(),
            conversation_id: None,
            workflow_id: Uuid::nil(),
            inputs: Default::default(),
            step_outputs: Default::default(),
            step_item_progress: Default::default(),
            extracted_path: PathBuf::from("/tmp"),
            sandbox_workspace: ws,
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
            persist_artifacts: false,
            force_log_capture: false,
            total_log_bytes: std::sync::atomic::AtomicU64::new(0),
        }
    }

    #[tokio::test]
    async fn skips_text_log_when_level_off() {
        let tmp = tempdir().unwrap();
        let ctx = fake_ctx(tmp.path().to_path_buf());
        let out =
            write_text_log(&ctx, "s", "prompt", "hi", LogCapture::Off).await.unwrap();
        assert!(out.is_none());
    }

    #[tokio::test]
    async fn writes_stderr_at_stderr_level() {
        let tmp = tempdir().unwrap();
        let ctx = fake_ctx(tmp.path().to_path_buf());
        let out =
            write_text_log(&ctx, "s", "stderr", "boom", LogCapture::Stderr).await.unwrap();
        let meta = out.unwrap();
        assert_eq!(meta.size_bytes, 4);
        assert_eq!(meta.preview, "boom");
    }

    #[tokio::test]
    async fn always_writes_trace() {
        let tmp = tempdir().unwrap();
        let ctx = fake_ctx(tmp.path().to_path_buf());
        let trace = StepTrace {
            started_at: Some(Utc::now()),
            completed_at: Some(Utc::now()),
            ms_elapsed: 100,
            tokens_used: 50,
            attempts: 1,
            on_error: None,
        };
        let m = write_trace(&ctx, "s", &trace).await.unwrap();
        assert!(m.path.exists());
    }

    #[tokio::test]
    async fn run_log_cap_marks_body_once_budget_spent() {
        // E7: once the per-run aggregate budget is spent, durable bodies become
        // a marker (the file on disk is still written; only the stored body caps).
        let tmp = tempdir().unwrap();
        let ctx = fake_ctx(tmp.path().to_path_buf());
        ctx.total_log_bytes.store(
            RUN_LOG_BODY_CAP_CHARS as u64,
            std::sync::atomic::Ordering::Relaxed,
        );
        let meta = write_text_log(&ctx, "s", "prompt", "some prompt body", LogCapture::Full)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(meta.body.as_deref(), Some("…[run log cap reached]"));
    }

    #[tokio::test]
    async fn run_log_cap_allows_and_charges_under_budget() {
        let tmp = tempdir().unwrap();
        let ctx = fake_ctx(tmp.path().to_path_buf());
        let meta = write_text_log(&ctx, "s", "prompt", "hello", LogCapture::Full)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(meta.body.as_deref(), Some("hello"));
        assert!(ctx.total_log_bytes.load(std::sync::atomic::Ordering::Relaxed) >= 5);
    }
}
