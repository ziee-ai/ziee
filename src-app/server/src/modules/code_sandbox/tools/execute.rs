//! `execute_command` — invokes bwrap with a user-supplied shell command.

use axum::http::StatusCode;
use serde_json::json;

use crate::common::AppError;
use crate::modules::code_sandbox::config;
use crate::modules::code_sandbox::sandbox::{run_in_sandbox, DEFAULT_TIMEOUT_SECS};
use crate::modules::code_sandbox::types::SandboxContext;

pub async fn execute_command(
    ctx: &SandboxContext,
    command: &str,
) -> Result<serde_json::Value, AppError> {
    let state = config::get_state().ok_or_else(|| {
        AppError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "SANDBOX_NOT_INITIALIZED",
            "code_sandbox state not initialized; module disabled or not yet booted",
        )
    })?;

    let result = run_in_sandbox(&state, ctx, command, Some(DEFAULT_TIMEOUT_SECS)).await?;

    Ok(json!({
        "stdout": result.stdout,
        "stderr": result.stderr,
        "exit_code": result.exit_code,
        "timed_out": result.timed_out,
        "duration_ms": result.duration_ms,
        "stdout_truncated": result.stdout_truncated,
        "stderr_truncated": result.stderr_truncated,
    }))
}
