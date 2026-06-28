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
use crate::modules::code_sandbox::mount_provider;
use crate::modules::code_sandbox::sandbox::DEFAULT_TIMEOUT_SECS;
use crate::modules::code_sandbox::types::{SandboxContext, CONVERSATION_FLAVOR};
use crate::modules::code_sandbox::version_manager;
use crate::modules::code_sandbox::workflow_staging::{StageMode, StagedMount};

pub async fn execute_command(
    ctx: &SandboxContext,
    command: &str,
    flavor: &str,
) -> Result<serde_json::Value, AppError> {
    execute_command_with_mounts(ctx, command, flavor, &[], None).await
}

/// `execute_command` with additional per-call bwrap binds (workflow
/// runner integration; B4) + an optional live-progress sink. Mounts are
/// partitioned by mode in `build_bwrap_argv` (RO → `--ro-bind`, RW →
/// `--bind`). When `progress_tx` is `Some`, the `/ziee/progress` FIFO is bound
/// into the sandbox and each newline-trimmed line code writes to
/// `$ZIEE_PROGRESS` (one raw FIFO `write()`) is forwarded to the sender — the
/// seam the workflow dispatcher consumes. The chat/MCP-side `execute_command`
/// calls this with `&[]` + `None`.
pub async fn execute_command_with_mounts(
    ctx: &SandboxContext,
    command: &str,
    flavor: &str,
    extra_mounts: &[StagedMount],
    progress_tx: Option<tokio::sync::mpsc::UnboundedSender<Vec<u8>>>,
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
    // Provider-contributed mounts (e.g. desktop host-folder mounts; feature #3
    // Part B). The seam is inert when no provider is registered (standalone /
    // remote-web server), so this is a cheap no-op there. Merged with the
    // workflow runner's `extra_mounts` — both flow through the same
    // `StagedMount` → `build_bwrap_argv` bind path.
    let (provider_mounts, mut mount_notes) = mount_provider::collect_and_sanitize(ctx).await;

    // Honest per-backend reporting: the VM backends (macOS/WSL2) don't yet bind
    // extra mounts (virtio-fs share / 9p carve-out are follow-ups), so surface a
    // note instead of silently dropping. Linux binds them directly.
    let applied_provider_mounts: Vec<StagedMount> = if backend::active().supports_extra_mounts() {
        provider_mounts
    } else {
        for m in &provider_mounts {
            mount_notes.push(format!(
                "host folder '{}' could not be mounted: host-folder mounting is not yet supported on this sandbox backend",
                m.sandbox_path
            ));
        }
        Vec::new()
    };
    let all_mounts: Vec<StagedMount> = extra_mounts
        .iter()
        .cloned()
        .chain(applied_provider_mounts.iter().cloned())
        .collect();

    let ensure = backend::active().ensure_rootfs_ready(&state, flavor).await?;
    let fetch_info = ensure.fetch_info.clone();

    let result = backend::active()
        .run_with_mounts(
            &state,
            ctx,
            command,
            Some(DEFAULT_TIMEOUT_SECS),
            flavor,
            &all_mounts,
            progress_tx,
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
    // Surface the active host-folder mounts so the model knows what's available
    // and where (read-through path resolution lives in the provider).
    if !applied_provider_mounts.is_empty() {
        response["mounts"] = json!(applied_provider_mounts
            .iter()
            .map(|m| json!({
                "path": m.sandbox_path,
                "read_only": m.mode == StageMode::ReadOnly,
            }))
            .collect::<Vec<_>>());
    }
    // Folders that were configured but unavailable/rejected at run time
    // (skip-with-note, not fatal).
    if !mount_notes.is_empty() {
        response["mount_notes"] = json!(mount_notes);
    }
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::code_sandbox::models::ConversationFile;
    use crate::modules::code_sandbox::types::{SandboxContext, CONVERSATION_FLAVOR};
    use std::path::PathBuf;
    use std::sync::Arc;
    use uuid::Uuid;

    fn ctx_for(conv: Uuid) -> SandboxContext {
        SandboxContext {
            conversation_id: conv,
            user_id: Uuid::new_v4(),
            workspace: PathBuf::from("/nonexistent-test-workspace"),
            files: Arc::new(Vec::<ConversationFile>::new()),
        }
    }

    /// The `SANDBOX_NOT_INITIALIZED` guard is the first thing
    /// `execute_command` does: when the module state is not initialized
    /// (the default in a lib unit-test process — no sandbox booted), the
    /// call must fail fast with a 503 / `SANDBOX_NOT_INITIALIZED` rather
    /// than panicking, touching the filesystem, or invoking bwrap. This
    /// pins both the status and the stable machine-readable error code.
    #[tokio::test]
    async fn execute_command_errors_when_state_uninitialized() {
        // This test is only meaningful while sandbox state is unset; in the
        // lib test binary nothing boots the module, so `get_state()` is None.
        if config::get_state().is_some() {
            return; // some other in-process test booted the sandbox; skip.
        }

        let conv = Uuid::new_v4();
        let ctx = ctx_for(conv);

        let err = execute_command(&ctx, "echo hi", "minimal")
            .await
            .expect_err("uninitialized sandbox must return an error, not Ok");

        assert_eq!(
            err.status_code(),
            StatusCode::SERVICE_UNAVAILABLE.as_u16(),
            "uninitialized sandbox should map to 503"
        );
        assert_eq!(err.error_code(), "SANDBOX_NOT_INITIALIZED");
    }

    /// Ordering guarantee: the not-initialized guard short-circuits BEFORE
    /// the per-conversation flavor lock is written. A regression that moved
    /// the `CONVERSATION_FLAVOR` insert (or the flavor-switch wipe) ahead of
    /// the state check would leave a stale lock entry behind even on a failed
    /// call — assert the conversation never gets pinned when the call errors.
    #[tokio::test]
    async fn failed_uninitialized_call_does_not_pin_conversation_flavor() {
        if config::get_state().is_some() {
            return;
        }

        let conv = Uuid::new_v4();
        // Precondition: this fresh conversation id has no flavor lock yet.
        assert!(CONVERSATION_FLAVOR.get(&conv).is_none());

        let ctx = ctx_for(conv);
        let _ = execute_command(&ctx, "echo hi", "minimal").await;

        assert!(
            CONVERSATION_FLAVOR.get(&conv).is_none(),
            "the early SANDBOX_NOT_INITIALIZED return must not pin a flavor lock"
        );
    }
}
