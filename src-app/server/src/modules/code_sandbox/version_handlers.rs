//! Admin REST surface for the rootfs version lifecycle (Plan 5
//! Phase 2c). Mirrors the local-llm-runtime admin pattern:
//! GitHub-Releases discovery + DB-backed install/pin/delete.
//!
//! Endpoints (all gated by the existing `code_sandbox::environments::*`
//! permission scopes — admins get them via the `*` wildcard):
//!
//!   * `GET    /code-sandbox/rootfs/versions`            — status
//!   * `POST   /code-sandbox/rootfs/versions/install`    — download artifact
//!   * `POST   /code-sandbox/rootfs/versions/set-pin`    — change the pin
//!   * `DELETE /code-sandbox/rootfs/versions/{id}`       — delete row + file
//!
//! Phase 4 (admin UI) wires these into a streaming SSE channel for
//! the install progress. For Phase 2c the install handler runs
//! synchronously — the resulting download blocks the response until
//! complete, same as the legacy `prefetch` POST. The new pinned-version
//! mental model is: admin downloads + pins are deliberate operations
//! the operator triggers from the UI; the chat-side auto-fetch already
//! has its own SSE-progress plumbing via `streaming.rs`.

use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::Json;
use futures_util::Stream;
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::common::ApiResult;
use crate::modules::code_sandbox::config;
use crate::modules::code_sandbox::permissions::{
    CodeSandboxEnvironmentsManage, CodeSandboxEnvironmentsRead,
};
use crate::modules::code_sandbox::version_install_tasks::{
    self, InstallTaskState, SSEInstallConnectedData, SSEInstallTaskEvent,
};
use crate::modules::code_sandbox::version_manager::{
    self, SwapOutcome, VersionStatus,
};
use crate::modules::permissions::openapi::with_permission;
use crate::modules::permissions::RequirePermissions;

// =====================================================================
// Request shapes
// =====================================================================

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct InstallVersionRequest {
    /// Semver string (no leading `v`), e.g. `"0.1.0"`.
    pub version: String,
    /// Host arch — `"x86_64"` or `"aarch64"`. Phase 4 will derive this
    /// from `std::env::consts::ARCH` in the UI; the admin can override
    /// for cross-host pre-stages.
    pub arch: String,
    /// `"minimal"` or `"full"`.
    pub flavor: String,
    /// `"squashfs"` (Linux/macOS) or `"tar.zst"` (Windows WSL).
    pub package: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct SetPinRequest {
    pub version: String,
}

// =====================================================================
// Helpers
// =====================================================================

/// Resolve the live DB pool from the global sandbox state. Returns a
/// 503 when the sandbox isn't initialized (e.g. `enabled: false` in
/// config) so the admin UI gets a clear error rather than a panic.
fn live_pool() -> Result<std::sync::Arc<sqlx::PgPool>, (StatusCode, crate::common::AppError)> {
    let state = config::get_state().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            crate::common::AppError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "SANDBOX_NOT_INITIALIZED",
                "code_sandbox is not initialized (enabled: false in config or boot probe failed)",
            ),
        )
    })?;
    let pool = state.pool.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            crate::common::AppError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "SANDBOX_POOL_MISSING",
                "code_sandbox state has no DB pool wired",
            ),
        )
    })?;
    Ok(pool.clone())
}

/// Derive the rootfs cache root (parent of the legacy `current`
/// symlink — same convention used by the legacy fetch path).
fn cache_root() -> Result<std::path::PathBuf, (StatusCode, crate::common::AppError)> {
    let state = config::get_state().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            crate::common::AppError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "SANDBOX_NOT_INITIALIZED",
                "code_sandbox is not initialized",
            ),
        )
    })?;
    Ok(std::path::PathBuf::from(state.config.rootfs_path())
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from(".")))
}

fn map_version_err(err: version_manager::VersionError) -> (StatusCode, crate::common::AppError) {
    err.to_app_error().to_api_error()
}

// =====================================================================
// GET /code-sandbox/rootfs/versions
// =====================================================================

pub async fn get_versions_handler(
    _auth: RequirePermissions<(CodeSandboxEnvironmentsRead,)>,
) -> ApiResult<Json<VersionStatus>> {
    let pool = live_pool()?;
    let status = version_manager::status(&pool).await.map_err(map_version_err)?;
    Ok((StatusCode::OK, Json(status)))
}

pub fn get_versions_docs(
    op: aide::transform::TransformOperation,
) -> aide::transform::TransformOperation {
    with_permission::<(CodeSandboxEnvironmentsRead,)>(op)
        .id("CodeSandbox.getRootfsVersions")
        .tag("Code Sandbox")
        .summary("List installed + available rootfs versions")
        .description(
            "Returns the system-wide pin, every downloaded artifact, and \
             (best-effort) the GitHub Releases catalog. The UI uses this \
             single call to render the rootfs-versions admin page.",
        )
        .response::<200, Json<VersionStatus>>()
}

// =====================================================================
// POST /code-sandbox/rootfs/versions/install
// =====================================================================

pub async fn install_version_handler(
    _auth: RequirePermissions<(CodeSandboxEnvironmentsManage,)>,
    Json(body): Json<InstallVersionRequest>,
) -> ApiResult<Json<InstallTaskState>> {
    let pool = live_pool()?;
    let root = cache_root()?;
    let state = version_install_tasks::start_install_task(
        (*pool).clone(),
        root,
        body.version,
        body.arch,
        body.flavor,
        body.package,
    );
    // 202 Accepted: the install runs in tokio::spawn; subscribers to
    // `/install/subscribe` see live progress.
    Ok((StatusCode::ACCEPTED, Json(state)))
}

pub fn install_version_docs(
    op: aide::transform::TransformOperation,
) -> aide::transform::TransformOperation {
    with_permission::<(CodeSandboxEnvironmentsManage,)>(op)
        .id("CodeSandbox.installRootfsVersion")
        .tag("Code Sandbox")
        .summary("Spawn a background install task for a rootfs artifact")
        .description(
            "Spawns a background task to download the matching artifact from \
             the GitHub release, sha256 + cosign verify it, and record the \
             row in `code_sandbox_rootfs_artifacts`. Returns 202 Accepted \
             immediately with the task's initial state; live progress is \
             available via `GET .../install/subscribe` (SSE).",
        )
        .response::<202, Json<InstallTaskState>>()
}

// =====================================================================
// GET /code-sandbox/rootfs/versions/install/subscribe (SSE)
// =====================================================================

pub async fn subscribe_install_progress_handler(
    _auth: RequirePermissions<(CodeSandboxEnvironmentsRead,)>,
) -> ApiResult<Sse<impl Stream<Item = Result<Event, axum::Error>>>> {
    use async_stream::stream;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let client_id = version_install_tasks::register_client(tx.clone());

    // Connected handshake (typed enum variant → axum Event via
    // `sse_event_enum!`-generated Into).
    version_install_tasks::send_to(
        &tx,
        SSEInstallTaskEvent::Connected(SSEInstallConnectedData {
            message: "connected to install task stream".to_string(),
        }),
    );

    // Replay the current registry so a fresh subscriber sees what's
    // already running (or recently finished) without waiting for the
    // next progress tick.
    for state in version_install_tasks::list_tasks() {
        version_install_tasks::send_to(&tx, SSEInstallTaskEvent::TaskState(state));
    }

    let stream = stream! {
        // Keep the local sender alive for the stream's lifetime so
        // the SSE_CLIENTS entry stays valid.
        let _tx_keeper = tx;
        while let Some(event) = rx.recv().await {
            yield event;
        }
        version_install_tasks::remove_client(client_id);
    };

    Ok((StatusCode::OK, Sse::new(stream).keep_alive(KeepAlive::default())))
}

pub fn subscribe_install_progress_docs(
    op: aide::transform::TransformOperation,
) -> aide::transform::TransformOperation {
    with_permission::<(CodeSandboxEnvironmentsRead,)>(op)
        .id("CodeSandbox.subscribeRootfsInstallProgress")
        .tag("Code Sandbox")
        .summary("Subscribe to rootfs install task progress (SSE)")
        .description(
            "Server-Sent Events stream of `connected | taskStarted | progress \
             | complete | failed | taskState` events for every install task. \
             On connect the stream emits a `connected` event then replays the \
             current registry (recent terminal states + in-flight tasks) so a \
             fresh subscriber doesn't have to wait for the next tick.",
        )
        .response::<200, Json<SSEInstallTaskEvent>>()
}

// =====================================================================
// POST /code-sandbox/rootfs/versions/set-pin
// =====================================================================

#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
pub struct SetPinResponse {
    pub swap: SwapOutcome,
    pub status: VersionStatus,
}

pub async fn set_pin_handler(
    _auth: RequirePermissions<(CodeSandboxEnvironmentsManage,)>,
    Json(body): Json<SetPinRequest>,
) -> ApiResult<Json<SetPinResponse>> {
    let pool = live_pool()?;
    let state = config::get_state().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            crate::common::AppError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "SANDBOX_NOT_INITIALIZED",
                "code_sandbox is not initialized",
            ),
        )
    })?;
    let swap = version_manager::set_pin_with_drain(
        &pool,
        &body.version,
        state.workspace_root.clone(),
    )
    .await
    .map_err(map_version_err)?;
    let status = version_manager::status(&pool).await.map_err(map_version_err)?;
    Ok((StatusCode::OK, Json(SetPinResponse { swap, status })))
}

pub fn set_pin_docs(
    op: aide::transform::TransformOperation,
) -> aide::transform::TransformOperation {
    with_permission::<(CodeSandboxEnvironmentsManage,)>(op)
        .id("CodeSandbox.setRootfsPin")
        .tag("Code Sandbox")
        .summary("Change the system-wide rootfs version pin")
        .description(
            "Validates the target version exists on GitHub, updates the \
             pin in `code_sandbox_settings`, then schedules a drain-then- \
             evict task for every old-version mount. On a major version \
             bump the workspace install-cache subdirs (`.local`, \
             `.cache`, `.npm`, ...) are wiped across both per-conversation \
             and per-MCP-server workspaces AFTER drain; minor + patch \
             bumps preserve workspace state. Returns the swap outcome \
             (draining-mount count + cache wipe policy) alongside the \
             refreshed status snapshot.",
        )
        .response::<200, Json<SetPinResponse>>()
}

// =====================================================================
// DELETE /code-sandbox/rootfs/versions/{id}
// =====================================================================

pub async fn delete_version_handler(
    _auth: RequirePermissions<(CodeSandboxEnvironmentsManage,)>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
) -> ApiResult<Json<VersionStatus>> {
    let pool = live_pool()?;
    version_manager::delete_artifact(&pool, id)
        .await
        .map_err(map_version_err)?;
    let status = version_manager::status(&pool).await.map_err(map_version_err)?;
    Ok((StatusCode::OK, Json(status)))
}

pub fn delete_version_docs(
    op: aide::transform::TransformOperation,
) -> aide::transform::TransformOperation {
    with_permission::<(CodeSandboxEnvironmentsManage,)>(op)
        .id("CodeSandbox.deleteRootfsVersion")
        .tag("Code Sandbox")
        .summary("Delete an installed rootfs artifact")
        .description(
            "Deletes the DB row + the on-disk artifact + sidecars. \
             Refused with 409 when the row is the currently-pinned \
             version (change the pin first).",
        )
        .response::<200, Json<VersionStatus>>()
}
