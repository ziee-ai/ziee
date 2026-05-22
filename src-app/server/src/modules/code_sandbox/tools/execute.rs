//! `execute_command` — invokes bwrap with a user-supplied shell command.

use axum::http::StatusCode;
use serde_json::json;

use crate::common::AppError;
use crate::modules::code_sandbox::backend;
use crate::modules::code_sandbox::config;
use crate::modules::code_sandbox::sandbox::DEFAULT_TIMEOUT_SECS;
use crate::modules::code_sandbox::types::{SandboxContext, CONVERSATION_FLAVOR};

pub async fn execute_command(
    ctx: &SandboxContext,
    command: &str,
    flavor: &str,
) -> Result<serde_json::Value, AppError> {
    let state = config::get_state().ok_or_else(|| {
        AppError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "SANDBOX_NOT_INITIALIZED",
            "code_sandbox state not initialized; module disabled or not yet booted",
        )
    })?;

    // Per-conversation flavor lock: the FIRST execute_command in a
    // conversation pins the flavor. Subsequent calls with a different
    // flavor are accepted but logged — they trigger a fresh mount
    // (runtime_mount handles concurrency; both flavors stay live for
    // the rest of the conversation).
    let locked_flavor = CONVERSATION_FLAVOR
        .entry(ctx.conversation_id)
        .or_insert_with(|| flavor.to_string())
        .clone();
    if locked_flavor != flavor {
        tracing::info!(
            conv = %ctx.conversation_id,
            requested = flavor,
            initial = locked_flavor.as_str(),
            "execute_command: flavor switch within conversation"
        );
    }

    // The user-visible "fetch_info" surface for the chat UI. Pre-fetch
    // the rootfs before run_in_sandbox so we can capture the fetch
    // outcome separately (run_in_sandbox internally calls
    // ensure_rootfs_ready again, but that's a cheap warm-path lookup).
    let ensure = backend::active().ensure_rootfs_ready(&state, flavor).await?;
    let fetch_info = ensure.fetch_info.clone();

    let result = backend::active()
        .run(&state, ctx, command, Some(DEFAULT_TIMEOUT_SECS), flavor)
        .await?;

    let mut response = json!({
        "stdout": result.stdout,
        "stderr": result.stderr,
        "exit_code": result.exit_code,
        "timed_out": result.timed_out,
        "duration_ms": result.duration_ms,
        "stdout_truncated": result.stdout_truncated,
        "stderr_truncated": result.stderr_truncated,
        "flavor": flavor,
    });
    if let Some(info) = fetch_info {
        response["fetch_info"] = json!({
            "flavor": flavor,
            "bytes_downloaded": info.bytes_downloaded,
            "duration_ms": info.duration_ms,
            "cosign_verified": info.cosign_verified,
        });
    }
    Ok(response)
}
