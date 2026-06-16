//! `execute_command` — invokes bwrap with a user-supplied shell command.
//!
//! `execute_command_with_mounts` (B4) is the additive entry point the
//! workflow runner's `SandboxDispatcher` calls — it threads per-call
//! `StagedMount`s through to `build_bwrap_argv`. Every other caller
//! continues to use the no-mounts `execute_command` and is unaffected.

use axum::http::StatusCode;
use serde_json::json;

use crate::common::AppError;
use crate::modules::code_sandbox::backend;
use crate::modules::code_sandbox::config;
use crate::modules::code_sandbox::sandbox::DEFAULT_TIMEOUT_SECS;
use crate::modules::code_sandbox::types::{SandboxContext, CONVERSATION_FLAVOR};
use crate::modules::code_sandbox::version_manager;
use crate::modules::code_sandbox::workflow_staging::StagedMount;

pub async fn execute_command(
    ctx: &SandboxContext,
    command: &str,
    flavor: &str,
) -> Result<serde_json::Value, AppError> {
    execute_command_with_mounts(ctx, command, flavor, &[]).await
}

/// `execute_command` with additional per-call bwrap binds (workflow
/// runner integration; B4). Mounts are partitioned by mode in
/// `build_bwrap_argv` (RO → `--ro-bind`, RW → `--bind`). The chat-side
/// `execute_command` calls this with `&[]`.
pub async fn execute_command_with_mounts(
    ctx: &SandboxContext,
    command: &str,
    flavor: &str,
    extra_mounts: &[StagedMount],
) -> Result<serde_json::Value, AppError> {
    let state = config::get_state().ok_or_else(|| {
        AppError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "SANDBOX_NOT_INITIALIZED",
            "code_sandbox state not initialized; module disabled or not yet booted",
        )
    })?;

    // Per-conversation workspace dir lives directly under the
    // workspace root (see handlers.rs::workspace_for).
    let workspace_dir = state
        .workspace_root
        .join(ctx.conversation_id.to_string());

    // Ensure the workspace dir exists with the correct mode for the
    // in-sandbox uid. The chat-side path does this in `build_context`,
    // but the workflow runner calls this fn directly — without the macOS
    // 0o1777 chmod, the in-VM uid 1001 can't create the `.gitconfig`
    // dotfile-mask mountpoint and bwrap fails with "Permission denied".
    // Shared helper keeps the H-3 mode policy in lockstep with build_context.
    let _ = tokio::fs::create_dir_all(&workspace_dir).await;
    crate::modules::code_sandbox::handlers::apply_workspace_mode(&workspace_dir).await;

    // Per-conversation flavor lock: the FIRST execute_command in a
    // conversation pins the flavor. Subsequent calls with a different
    // flavor trigger Trigger B (per-conversation install-cache wipe)
    // before the new flavor runs — protects the LLM from ABI
    // mismatches between Python/Node libraries baked against the
    // previous flavor.
    let locked_flavor = CONVERSATION_FLAVOR
        .entry(ctx.conversation_id)
        .or_insert_with(|| flavor.to_string())
        .clone();
    if locked_flavor != flavor {
        tracing::info!(
            conv = %ctx.conversation_id,
            requested = flavor,
            previous = locked_flavor.as_str(),
            "execute_command: flavor switch within conversation — wiping install-cache subdirs"
        );
        let wipe = version_manager::wipe_install_caches_for_conversation(
            &workspace_dir,
            &locked_flavor,
            flavor,
        );
        tracing::info!(
            conv = %ctx.conversation_id,
            subdirs_removed = wipe.subdirs_removed,
            "execute_command: flavor-switch wipe complete"
        );
        // Update the lock so the next call sees the new flavor as the
        // baseline.
        CONVERSATION_FLAVOR.insert(ctx.conversation_id, flavor.to_string());
    }

    // Read + unlink any pending wipe sentinel from a previous
    // major-bump or flavor-switch wipe — `system_note` (if Some) will
    // be prepended to the tool result so the LLM knows to reinstall.
    let system_note = version_manager::consume_workspace_sentinel(&workspace_dir);

    // The user-visible "fetch_info" surface for the chat UI. Pre-fetch
    // the rootfs before run_in_sandbox so we can capture the fetch
    // outcome separately (run_in_sandbox internally calls
    // ensure_rootfs_ready again, but that's a cheap warm-path lookup).
    let ensure = backend::active().ensure_rootfs_ready(&state, flavor).await?;
    let fetch_info = ensure.fetch_info.clone();

    let result = backend::active()
        .run_with_mounts(
            &state,
            ctx,
            command,
            Some(DEFAULT_TIMEOUT_SECS),
            flavor,
            extra_mounts,
        )
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
            "version": info.version,
            "flavor": flavor,
            "bytes_downloaded": info.bytes_downloaded,
            "duration_ms": info.duration_ms,
            "cosign_verified": info.cosign_verified,
        });
    }
    if let Some(note) = system_note {
        response["system_note"] = json!(note);
    }
    Ok(response)
}
