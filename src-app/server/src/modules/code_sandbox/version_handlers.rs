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
use axum::Json;
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::common::ApiResult;
use crate::modules::code_sandbox::config;
use crate::modules::code_sandbox::permissions::{
    CodeSandboxEnvironmentsManage, CodeSandboxEnvironmentsRead,
};
use crate::modules::code_sandbox::version_manager::{
    self, RootfsArtifact, VersionStatus,
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
) -> ApiResult<Json<RootfsArtifact>> {
    let pool = live_pool()?;
    let root = cache_root()?;
    let (artifact, _stats) = version_manager::install_version(
        &pool,
        &root,
        &body.version,
        &body.arch,
        &body.flavor,
        &body.package,
        |_| {},
    )
    .await
    .map_err(map_version_err)?;
    Ok((StatusCode::OK, Json(artifact)))
}

pub fn install_version_docs(
    op: aide::transform::TransformOperation,
) -> aide::transform::TransformOperation {
    with_permission::<(CodeSandboxEnvironmentsManage,)>(op)
        .id("CodeSandbox.installRootfsVersion")
        .tag("Code Sandbox")
        .summary("Download + register a rootfs artifact")
        .description(
            "Downloads the matching artifact from the GitHub release, \
             sha256 + cosign verifies it, and records the row in \
             `code_sandbox_rootfs_artifacts`. Idempotent: a hash-matched \
             cache hit returns the existing row without touching the \
             network.",
        )
        .response::<200, Json<RootfsArtifact>>()
}

// =====================================================================
// POST /code-sandbox/rootfs/versions/set-pin
// =====================================================================

pub async fn set_pin_handler(
    _auth: RequirePermissions<(CodeSandboxEnvironmentsManage,)>,
    Json(body): Json<SetPinRequest>,
) -> ApiResult<Json<VersionStatus>> {
    let pool = live_pool()?;
    version_manager::set_pin(&pool, &body.version)
        .await
        .map_err(map_version_err)?;
    let status = version_manager::status(&pool).await.map_err(map_version_err)?;
    Ok((StatusCode::OK, Json(status)))
}

pub fn set_pin_docs(
    op: aide::transform::TransformOperation,
) -> aide::transform::TransformOperation {
    with_permission::<(CodeSandboxEnvironmentsManage,)>(op)
        .id("CodeSandbox.setRootfsPin")
        .tag("Code Sandbox")
        .summary("Change the system-wide rootfs version pin")
        .description(
            "Validates the target version exists on GitHub, then writes \
             it into `code_sandbox_settings.current_rootfs_version`. \
             Phase 2 ships the pin update only; Phase 3 will wrap this \
             with drain + (on major bump) install-cache wipe.",
        )
        .response::<200, Json<VersionStatus>>()
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
