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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntryMeta {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub preview: String,
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
    if !should_capture(kind, log_level) {
        return Ok(None);
    }
    let dir = step_log_dir(ctx, step_id);
    ensure_dir(&dir).await?;
    let dest = dir.join(kind);
    tokio::fs::write(&dest, body.as_bytes()).await.map_err(|e| {
        AppError::internal_error(format!("log_io: write {}: {e}", dest.display()))
    })?;
    let size = body.len() as u64;
    let preview = body
        .chars()
        .take(500)
        .collect::<String>();
    Ok(Some(LogEntryMeta {
        path: dest,
        size_bytes: size,
        preview,
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
    if log_level != LogCapture::Full {
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
    let preview = String::from_utf8_lossy(&bytes[..bytes.len().min(500)]).into_owned();
    Ok(Some(LogEntryMeta {
        path: dest,
        size_bytes: bytes.len() as u64,
        preview,
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
    let preview = String::from_utf8_lossy(&bytes[..bytes.len().min(500)]).into_owned();
    Ok(LogEntryMeta {
        path: dest,
        size_bytes: bytes.len() as u64,
        preview,
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
            sandbox_flavor: None,
            total_tokens: 0,
            total_output_bytes: 0,
            is_dev: false,
            mocks: std::collections::HashMap::new(),
            force_mocks: false,
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
}
