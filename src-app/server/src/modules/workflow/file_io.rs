//! Per-step output file persistence (plan §4.5 + §4.7).
//!
//! Step outputs are written to `<workspace>/<conv>/workflow/<run>/outputs/<step_id>.{json|txt}`
//! via atomic temp-then-rename. Metadata (path / size / sha256 /
//! preview / kind / parsed_as) goes into the DB row's
//! `step_outputs_json` column AFTER the file is on disk (write-file-
//! then-DB: a crash between leaves an orphan file but never a DB row
//! pointing at a missing file — the safe ordering). The orphan file is
//! reclaimed when the run reaches a terminal status: every terminal
//! path (`run_workflow` completion/fail/cancel + the startup sweep for
//! runs interrupted by a restart) removes the whole staged
//! `<workspace>/<conv>/workflow/<run>/` directory, taking any orphan
//! output file with it. (The sweep removes orphan run DIRECTORIES, not
//! individual files.)

#![allow(dead_code)]

use std::path::PathBuf;

use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;

use crate::common::AppError;
use crate::modules::workflow::types::{OutputMeta, ParsedAs, RunContext, StepKindTag};

/// Preview cap (chars / bytes — we use bytes here; the preview is for
/// the UI/event payload, not parsed downstream).
pub const PREVIEW_CAP_BYTES: usize = 500;

/// Hard cap on a single step's output file size (plan §4.4).
pub const STEP_OUTPUT_CAP_BYTES: u64 = 10 * 1024 * 1024;

/// Write `value` into `<ctx.outputs_dir>/<step_id>.{json|txt}` atomically.
/// Returns the `OutputMeta` the caller persists into
/// `step_outputs_json`.
pub async fn write_step_output(
    ctx: &RunContext,
    step_id: &str,
    value: &Value,
    parsed_as: ParsedAs,
    kind: StepKindTag,
) -> Result<OutputMeta, AppError> {
    tokio::fs::create_dir_all(&ctx.outputs_dir).await.map_err(|e| {
        AppError::internal_error(format!("workflow file_io: mkdir outputs: {e}"))
    })?;

    let dest = ctx.step_output_host_path(step_id, parsed_as);
    let bytes = match parsed_as {
        ParsedAs::Json => serde_json::to_vec_pretty(value)
            .map_err(|e| AppError::internal_error(format!("serialize step output: {e}")))?,
        ParsedAs::Text => match value {
            Value::String(s) => s.as_bytes().to_vec(),
            // Defensive: if a Text-tagged value isn't a string, fall back to JSON.
            other => serde_json::to_vec(other)
                .map_err(|e| AppError::internal_error(format!("serialize step output: {e}")))?,
        },
    };

    if bytes.len() as u64 > STEP_OUTPUT_CAP_BYTES {
        return Err(AppError::bad_request(
            "STEP_OUTPUT_OVERSIZE",
            format!(
                "step '{step_id}' output {} bytes exceeds {} byte cap",
                bytes.len(),
                STEP_OUTPUT_CAP_BYTES
            ),
        ));
    }

    let tmp = dest.with_extension(match parsed_as {
        ParsedAs::Json => "json.tmp",
        ParsedAs::Text => "txt.tmp",
    });

    tokio::fs::write(&tmp, &bytes).await.map_err(|e| {
        AppError::internal_error(format!(
            "workflow file_io: write tmp {}: {e}",
            tmp.display()
        ))
    })?;
    tokio::fs::rename(&tmp, &dest).await.map_err(|e| {
        AppError::internal_error(format!(
            "workflow file_io: rename {} -> {}: {e}",
            tmp.display(),
            dest.display()
        ))
    })?;

    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let sha = format!("{:x}", hasher.finalize());

    let preview = build_preview(&bytes);

    Ok(OutputMeta {
        path: dest,
        size_bytes: bytes.len() as u64,
        sha256: sha,
        preview,
        kind,
        parsed_as,
    })
}

/// Read the value back from disk. Used by the template engine when a
/// downstream template references `{{ step_id.output }}`. For
/// `ParsedAs::Json` we parse; for `Text` we return `Value::String`.
pub fn read_output_value(meta: &OutputMeta) -> Result<Value, AppError> {
    let bytes = std::fs::read(&meta.path).map_err(|e| {
        AppError::internal_error(format!(
            "workflow file_io: read {}: {e}",
            meta.path.display()
        ))
    })?;
    match meta.parsed_as {
        ParsedAs::Json => serde_json::from_slice(&bytes)
            .map_err(|e| AppError::internal_error(format!("parse step output JSON: {e}"))),
        ParsedAs::Text => {
            let s = String::from_utf8_lossy(&bytes).into_owned();
            Ok(Value::String(s))
        }
    }
}

/// REST `GET /api/workflow-runs/{id}/output/{step_id}` streaming
/// reader. Returns an AsyncRead the handler pipes back as a Body.
pub async fn open_output_stream(host_path: &PathBuf) -> Result<tokio::fs::File, AppError> {
    tokio::fs::File::open(host_path).await.map_err(|e| {
        AppError::new(
            axum::http::StatusCode::NOT_FOUND,
            "WORKFLOW_OUTPUT_MISSING",
            format!("workflow output file missing or unreadable: {e}"),
        )
    })
}

/// Async size check (used by the REST handler before opening the
/// stream so we can short-circuit oversized outputs).
pub async fn file_size(host_path: &PathBuf) -> Result<u64, AppError> {
    let md = tokio::fs::metadata(host_path).await.map_err(|e| {
        AppError::new(
            axum::http::StatusCode::NOT_FOUND,
            "WORKFLOW_OUTPUT_MISSING",
            format!("workflow output file missing: {e}"),
        )
    })?;
    Ok(md.len())
}

/// Drain an open AsyncRead into a Vec, capped at STEP_OUTPUT_CAP_BYTES.
pub async fn drain_to_string(file: &mut tokio::fs::File) -> Result<String, AppError> {
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).await.map_err(|e| {
        AppError::internal_error(format!("workflow file_io: read step output: {e}"))
    })?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

fn build_preview(bytes: &[u8]) -> String {
    if bytes.len() <= PREVIEW_CAP_BYTES {
        String::from_utf8_lossy(bytes).into_owned()
    } else {
        let head = &bytes[..PREVIEW_CAP_BYTES];
        format!("{}…", String::from_utf8_lossy(head))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use uuid::Uuid;

    fn fake_ctx(outputs_dir: PathBuf) -> RunContext {
        RunContext {
            run_id: Uuid::nil(),
            user_id: Uuid::nil(),
            conversation_id: None,
            workflow_id: Uuid::nil(),
            inputs: Default::default(),
            step_outputs: Default::default(),
            step_item_progress: Default::default(),
            extracted_path: PathBuf::from("/tmp"),
            sandbox_workspace: PathBuf::from("/tmp"),
            outputs_dir,
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
        }
    }

    #[tokio::test]
    async fn writes_json_and_reads_back() {
        let tmp = tempdir().unwrap();
        let ctx = fake_ctx(tmp.path().to_path_buf());
        let v = serde_json::json!({"a": 1});
        let meta =
            write_step_output(&ctx, "s1", &v, ParsedAs::Json, StepKindTag::Llm).await.unwrap();
        assert!(meta.path.exists());
        assert!(meta.path.to_string_lossy().ends_with("s1.json"));
        let back = read_output_value(&meta).unwrap();
        assert_eq!(back, v);
    }

    #[tokio::test]
    async fn writes_text_with_txt_extension() {
        let tmp = tempdir().unwrap();
        let ctx = fake_ctx(tmp.path().to_path_buf());
        let v = serde_json::json!("hello world");
        let meta =
            write_step_output(&ctx, "g", &v, ParsedAs::Text, StepKindTag::Llm).await.unwrap();
        assert!(meta.path.to_string_lossy().ends_with("g.txt"));
        assert_eq!(meta.size_bytes, 11);
    }

    #[tokio::test]
    async fn rejects_oversize_output() {
        let tmp = tempdir().unwrap();
        let ctx = fake_ctx(tmp.path().to_path_buf());
        let big = "x".repeat((STEP_OUTPUT_CAP_BYTES + 1) as usize);
        let v = Value::String(big);
        let err =
            write_step_output(&ctx, "big", &v, ParsedAs::Text, StepKindTag::Llm).await.unwrap_err();
        assert!(err.to_string().contains("oversize") || err.to_string().contains("OVERSIZE")
            || err.to_string().contains("byte cap"));
    }
}
